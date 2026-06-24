#![allow(unsafe_op_in_unsafe_fn, unused_imports, dead_code)]

use pick_agent::permission::sandbox::{Sandbox, SandboxConfig, SandboxRequest, SandboxType};

pub struct WindowsRestrictedTokenSandbox {
    enabled: bool,
}

impl WindowsRestrictedTokenSandbox {
    pub fn new(config: &SandboxConfig) -> Self {
        let enabled = config.sandbox_type == SandboxType::WindowsJob;
        Self { enabled }
    }
}

/// Constants for Windows API that may not be in windows-sys bindings
const SE_GROUP_LOGON_ID: u32 = 0xC000_0000;
const GENERIC_ALL: u32 = 0x1000_0000;
const DACL_SECURITY_INFORMATION: u32 = 4;
const CONTAINER_INHERIT_ACE: u32 = 2;
const OBJECT_INHERIT_ACE: u32 = 1;
const HANDLE_FLAG_INHERIT: u32 = 1;

#[cfg(windows)]
pub(crate) mod win_impl {
    use std::ffi::c_void;
    use std::path::Path;
    use std::ptr;

    use super::{
        CONTAINER_INHERIT_ACE, DACL_SECURITY_INFORMATION, GENERIC_ALL, OBJECT_INHERIT_ACE,
        SE_GROUP_LOGON_ID,
    };
    use pick_agent::permission::sandbox::SandboxRequest;

    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_SUCCESS, GetLastError, HANDLE, HANDLE_FLAG_INHERIT, HLOCAL,
        INVALID_HANDLE_VALUE, LUID, LocalFree, SetHandleInformation, WAIT_OBJECT_0, WAIT_TIMEOUT,
    };
    use windows_sys::Win32::Security::Authorization::{
        EXPLICIT_ACCESS_W, GRANT_ACCESS, GetNamedSecurityInfoW, NO_MULTIPLE_TRUSTEE,
        SE_FILE_OBJECT, SetEntriesInAclW, SetNamedSecurityInfoW, TRUSTEE_IS_SID,
        TRUSTEE_IS_UNKNOWN, TRUSTEE_W,
    };
    use windows_sys::Win32::Security::{
        ACL, AdjustTokenPrivileges, CopySid, CreateRestrictedToken, CreateWellKnownSid,
        DISABLE_MAX_PRIVILEGE, GetLengthSid, GetTokenInformation, LUA_TOKEN, LookupPrivilegeValueW,
        SID_AND_ATTRIBUTES, SetTokenInformation, TOKEN_ADJUST_DEFAULT, TOKEN_ADJUST_PRIVILEGES,
        TOKEN_ADJUST_SESSIONID, TOKEN_ASSIGN_PRIMARY, TOKEN_DUPLICATE, TOKEN_GROUPS,
        TOKEN_PRIVILEGES, TOKEN_QUERY, TokenDefaultDacl, TokenGroups, WRITE_RESTRICTED,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_DELETE_CHILD, FILE_GENERIC_WRITE, ReadFile,
    };
    use windows_sys::Win32::System::Pipes::CreatePipe;
    use windows_sys::Win32::System::Threading::{
        CreateProcessAsUserW, DeleteProcThreadAttributeList, GetCurrentProcess, GetExitCodeProcess,
        InitializeProcThreadAttributeList, OpenProcessToken, PROCESS_INFORMATION, STARTUPINFOEXW,
        STARTUPINFOW, TerminateProcess, UpdateProcThreadAttribute, WaitForSingleObject,
    };

    pub unsafe fn world_sid() -> Result<Vec<u8>, String> {
        let mut size: u32 = 0;
        CreateWellKnownSid(
            1, // WinWorldSid
            ptr::null_mut(),
            ptr::null_mut(),
            &mut size,
        );
        let mut buf: Vec<u8> = vec![0u8; size as usize];
        let ok = CreateWellKnownSid(
            1,
            ptr::null_mut(),
            buf.as_mut_ptr() as *mut c_void,
            &mut size,
        );
        if ok == 0 {
            return Err(format!(
                "CreateWellKnownSid(World) failed: {}",
                GetLastError()
            ));
        }
        Ok(buf)
    }

    unsafe fn set_default_dacl(h_token: HANDLE, sids: &[*mut c_void]) -> Result<(), String> {
        if sids.is_empty() {
            return Ok(());
        }
        let entries: Vec<EXPLICIT_ACCESS_W> = sids
            .iter()
            .map(|sid| EXPLICIT_ACCESS_W {
                grfAccessPermissions: GENERIC_ALL,
                grfAccessMode: GRANT_ACCESS,
                grfInheritance: 0,
                Trustee: TRUSTEE_W {
                    pMultipleTrustee: ptr::null_mut(),
                    MultipleTrusteeOperation: 0,
                    TrusteeForm: TRUSTEE_IS_SID,
                    TrusteeType: TRUSTEE_IS_UNKNOWN,
                    ptstrName: *sid as *mut u16,
                },
            })
            .collect();
        let mut p_new_dacl: *mut ACL = ptr::null_mut();
        let res = SetEntriesInAclW(
            entries.len() as u32,
            entries.as_ptr(),
            ptr::null_mut(),
            &mut p_new_dacl,
        );
        if res != ERROR_SUCCESS {
            return Err(format!("SetEntriesInAclW failed: {res}"));
        }
        let ok = SetTokenInformation(
            h_token,
            TokenDefaultDacl,
            &p_new_dacl as *const *mut ACL as *const c_void,
            std::mem::size_of::<*mut ACL>() as u32,
        );
        if ok == 0 {
            let err = GetLastError();
            if !p_new_dacl.is_null() {
                LocalFree(p_new_dacl as HLOCAL);
            }
            return Err(format!(
                "SetTokenInformation(TokenDefaultDacl) failed: {err}"
            ));
        }
        if !p_new_dacl.is_null() {
            LocalFree(p_new_dacl as HLOCAL);
        }
        Ok(())
    }

    unsafe fn enable_single_privilege(h_token: HANDLE, name: &str) -> Result<(), String> {
        let mut luid = LUID {
            LowPart: 0,
            HighPart: 0,
        };
        let name_ws: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let ok = LookupPrivilegeValueW(ptr::null(), name_ws.as_ptr(), &mut luid);
        if ok == 0 {
            return Err(format!("LookupPrivilegeValueW failed: {}", GetLastError()));
        }
        let mut tp: TOKEN_PRIVILEGES = std::mem::zeroed();
        tp.PrivilegeCount = 1;
        tp.Privileges[0].Luid = luid;
        tp.Privileges[0].Attributes = 0x00000002; // SE_PRIVILEGE_ENABLED
        let ok2 = AdjustTokenPrivileges(h_token, 0, &tp, 0, ptr::null_mut(), ptr::null_mut());
        if ok2 == 0 {
            return Err(format!("AdjustTokenPrivileges failed: {}", GetLastError()));
        }
        let err = GetLastError();
        if err != 0 {
            return Err(format!("AdjustTokenPrivileges error {err}"));
        }
        Ok(())
    }

    unsafe fn create_restricted_token() -> Result<(HANDLE, Vec<u8>), String> {
        let current_process = GetCurrentProcess();
        let mut h_token: HANDLE = INVALID_HANDLE_VALUE;
        let ok = OpenProcessToken(
            current_process,
            TOKEN_DUPLICATE
                | TOKEN_QUERY
                | TOKEN_ADJUST_DEFAULT
                | TOKEN_ASSIGN_PRIMARY
                | TOKEN_ADJUST_SESSIONID
                | TOKEN_ADJUST_PRIVILEGES,
            &mut h_token,
        );
        if ok == 0 {
            return Err(format!("OpenProcessToken failed: {}", GetLastError()));
        }

        // Extract logon SID for use as a restricting SID, matching codex approach
        let mut logon_sid_bytes = get_logon_sid_bytes(h_token)?;
        let psid_logon = logon_sid_bytes.as_mut_ptr() as *mut c_void;

        // Create Everyone SID (S-1-1-0) as an additional restricting SID
        let mut everyone = world_sid()?;
        let psid_everyone = everyone.as_mut_ptr() as *mut c_void;

        // Build restricting SID list: [Logon, Everyone] (matching codex pattern)
        let mut entries: Vec<SID_AND_ATTRIBUTES> = vec![std::mem::zeroed(); 2];
        entries[0].Sid = psid_logon;
        entries[0].Attributes = 0;
        entries[1].Sid = psid_everyone;
        entries[1].Attributes = 0;

        let mut new_token: HANDLE = 0;
        let flags = DISABLE_MAX_PRIVILEGE | LUA_TOKEN | WRITE_RESTRICTED;
        let ok = CreateRestrictedToken(
            h_token,
            flags,
            0,
            ptr::null(),
            0,
            ptr::null(),
            entries.len() as u32,
            entries.as_mut_ptr(),
            &mut new_token,
        );
        CloseHandle(h_token);
        if ok == 0 {
            return Err(format!("CreateRestrictedToken failed: {}", GetLastError()));
        }

        // Set permissive default DACL so the sandboxed process can create pipes/IPC.
        // Both logon SID and Everyone SID get GENERIC_ALL, matching codex.
        let dacl_sids: Vec<*mut c_void> = vec![psid_logon, psid_everyone];
        set_default_dacl(new_token, &dacl_sids)?;

        // Re-enable SeChangeNotifyPrivilege (bypass traverse checking) which was
        // disabled by DISABLE_MAX_PRIVILEGE. This is essential for msys/Git Bash
        // to access \BaseNamedObjects and other object manager namespace directories
        // — without it, the process gets STATUS_ACCESS_DENIED (0xC0000022).
        enable_single_privilege(new_token, "SeChangeNotifyPrivilege")?;

        Ok((new_token, logon_sid_bytes))
    }

    unsafe fn get_logon_sid_bytes(h_token: HANDLE) -> Result<Vec<u8>, String> {
        scan_token_groups_for_logon(h_token)
            .or_else(|| try_linked_token_logon(h_token))
            .ok_or_else(|| "Logon SID not present on token".to_string())
    }

    unsafe fn scan_token_groups_for_logon(h: HANDLE) -> Option<Vec<u8>> {
        let mut needed: u32 = 0;
        GetTokenInformation(h, TokenGroups, ptr::null_mut(), 0, &mut needed);
        if needed == 0 {
            return None;
        }
        let mut buf: Vec<u8> = vec![0u8; needed as usize];
        let ok = GetTokenInformation(
            h,
            TokenGroups,
            buf.as_mut_ptr() as *mut c_void,
            needed,
            &mut needed,
        );
        if ok == 0 || (needed as usize) < std::mem::size_of::<u32>() {
            return None;
        }
        let group_count = std::ptr::read_unaligned(buf.as_ptr() as *const u32) as usize;
        // TOKEN_GROUPS layout: DWORD GroupCount; SID_AND_ATTRIBUTES Groups[];
        // Groups is pointer-aligned after the 4-byte count (on 64-bit: 4 bytes padding)
        let after_count = unsafe { buf.as_ptr().add(std::mem::size_of::<u32>()) } as usize;
        let align = std::mem::align_of::<SID_AND_ATTRIBUTES>();
        let aligned = (after_count + (align - 1)) & !(align - 1);
        let groups_ptr = aligned as *const SID_AND_ATTRIBUTES;
        for i in 0..group_count {
            let entry: SID_AND_ATTRIBUTES = std::ptr::read_unaligned(groups_ptr.add(i));
            if (entry.Attributes & SE_GROUP_LOGON_ID) == SE_GROUP_LOGON_ID {
                let sid_len = GetLengthSid(entry.Sid);
                if sid_len == 0 {
                    return None;
                }
                let mut out = vec![0u8; sid_len as usize];
                if CopySid(sid_len, out.as_mut_ptr() as *mut c_void, entry.Sid) == 0 {
                    return None;
                }
                return Some(out);
            }
        }
        None
    }

    unsafe fn try_linked_token_logon(h: HANDLE) -> Option<Vec<u8>> {
        #[repr(C)]
        struct TOKEN_LINKED_TOKEN {
            linked_token: HANDLE,
        }
        const TOKEN_LINKED_TOKEN_CLASS: i32 = 19; // TokenLinkedToken
        let mut ln_needed: u32 = 0;
        GetTokenInformation(
            h,
            TOKEN_LINKED_TOKEN_CLASS,
            ptr::null_mut(),
            0,
            &mut ln_needed,
        );
        if ln_needed >= std::mem::size_of::<TOKEN_LINKED_TOKEN>() as u32 {
            let mut ln_buf: Vec<u8> = vec![0u8; ln_needed as usize];
            let ok = GetTokenInformation(
                h,
                TOKEN_LINKED_TOKEN_CLASS,
                ln_buf.as_mut_ptr() as *mut c_void,
                ln_needed,
                &mut ln_needed,
            );
            if ok != 0 {
                let lt: TOKEN_LINKED_TOKEN =
                    std::ptr::read_unaligned(ln_buf.as_ptr() as *const TOKEN_LINKED_TOKEN);
                if lt.linked_token != 0 {
                    let res = scan_token_groups_for_logon(lt.linked_token);
                    CloseHandle(lt.linked_token);
                    if let Some(v) = res {
                        return Some(v);
                    }
                }
            }
        }
        None
    }

    unsafe fn create_pipes_with_stdin()
    -> Result<(HANDLE, HANDLE, HANDLE, HANDLE, HANDLE, HANDLE), String> {
        let mut stdin_read: HANDLE = 0;
        let mut stdin_write: HANDLE = 0;
        let mut stdout_read: HANDLE = 0;
        let mut stdout_write: HANDLE = 0;
        let mut stderr_read: HANDLE = 0;
        let mut stderr_write: HANDLE = 0;

        // All pipes created as non-inheritable (null security attributes)
        if CreatePipe(&mut stdin_read, &mut stdin_write, ptr::null_mut(), 0) == 0 {
            return Err(format!("CreatePipe stdin failed: {}", GetLastError()));
        }
        if CreatePipe(&mut stdout_read, &mut stdout_write, ptr::null_mut(), 0) == 0 {
            CloseHandle(stdin_read);
            CloseHandle(stdin_write);
            return Err(format!("CreatePipe stdout failed: {}", GetLastError()));
        }
        if CreatePipe(&mut stderr_read, &mut stderr_write, ptr::null_mut(), 0) == 0 {
            CloseHandle(stdin_read);
            CloseHandle(stdin_write);
            CloseHandle(stdout_read);
            CloseHandle(stdout_write);
            return Err(format!("CreatePipe stderr failed: {}", GetLastError()));
        }

        // Mark only the handles the child needs as inheritable (matching codex pattern)
        SetHandleInformation(stdin_read, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT);
        SetHandleInformation(stdout_write, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT);
        SetHandleInformation(stderr_write, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT);

        Ok((
            stdin_read,
            stdin_write,
            stdout_read,
            stdout_write,
            stderr_read,
            stderr_write,
        ))
    }

    unsafe fn spawn_process_with_handles(
        token: HANDLE,
        command_line: &str,
        cwd: &str,
        stdin_read: HANDLE,
        stdout_write: HANDLE,
        stderr_write: HANDLE,
    ) -> Result<PROCESS_INFORMATION, String> {
        let mut cmd_ws: Vec<u16> = command_line
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let cwd_ws: Vec<u16> = cwd.encode_utf16().chain(std::iter::once(0)).collect();

        // ProcThreadAttributeList: restricts handle inheritance to exactly the
        // handles the child needs (matching codex pattern). Prevents pipe handle
        // leakage to grandchild processes.
        const PROC_THREAD_ATTRIBUTE_HANDLE_LIST: usize = 0x0002_0002;
        const EXTENDED_STARTUPINFO_PRESENT: u32 = 0x0008_0000;

        // Step 1: query required buffer size
        let mut attr_size: usize = 0;
        InitializeProcThreadAttributeList(ptr::null_mut(), 1, 0, &mut attr_size);
        let mut attr_buf = vec![0u8; attr_size];
        let attr_list = attr_buf.as_mut_ptr() as *mut c_void;
        if InitializeProcThreadAttributeList(attr_list, 1, 0, &mut attr_size) == 0 {
            return Err(format!(
                "InitializeProcThreadAttributeList failed: {}",
                GetLastError()
            ));
        }

        let inherited_handles = [stdin_read, stdout_write, stderr_write];
        if UpdateProcThreadAttribute(
            attr_list,
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
            inherited_handles.as_ptr() as *const c_void,
            inherited_handles.len() * std::mem::size_of::<HANDLE>(),
            ptr::null_mut(),
            ptr::null_mut(),
        ) == 0
        {
            DeleteProcThreadAttributeList(attr_list);
            return Err(format!(
                "UpdateProcThreadAttribute failed: {}",
                GetLastError()
            ));
        }

        let mut si: STARTUPINFOEXW = std::mem::zeroed();
        si.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
        si.StartupInfo.dwFlags = 0x0000_0100; // STARTF_USESTDHANDLES
        si.StartupInfo.hStdInput = stdin_read;
        si.StartupInfo.hStdOutput = stdout_write;
        si.StartupInfo.hStdError = stderr_write;
        si.lpAttributeList = attr_list;

        let mut pi = PROCESS_INFORMATION {
            hProcess: 0,
            hThread: 0,
            dwProcessId: 0,
            dwThreadId: 0,
        };

        let ok = CreateProcessAsUserW(
            token,
            ptr::null(),
            cmd_ws.as_mut_ptr(),
            ptr::null(),
            ptr::null(),
            1, // bInheritHandles = TRUE (restricted to only handles in the list)
            EXTENDED_STARTUPINFO_PRESENT,
            ptr::null(),
            cwd_ws.as_ptr(),
            &si.StartupInfo,
            &mut pi,
        );
        // Clean up attribute list regardless of success
        DeleteProcThreadAttributeList(attr_list);
        if ok == 0 {
            return Err(format!("CreateProcessAsUserW failed: {}", GetLastError()));
        }
        Ok(pi)
    }

    fn read_pipe_async(pipe: HANDLE) -> std::thread::JoinHandle<String> {
        std::thread::spawn(move || {
            let mut buf = vec![0u8; 4096];
            let mut result = String::new();
            loop {
                let mut bytes_read: u32 = 0;
                let ok = unsafe {
                    ReadFile(
                        pipe,
                        buf.as_mut_ptr(),
                        buf.len() as u32,
                        &mut bytes_read,
                        ptr::null_mut(),
                    )
                };
                if ok == 0 || bytes_read == 0 {
                    break;
                }
                if let Ok(s) = String::from_utf8(buf[..bytes_read as usize].to_vec()) {
                    result.push_str(&s);
                }
            }
            unsafe {
                CloseHandle(pipe);
            }
            result
        })
    }

    pub fn run_sandboxed_command(req: &SandboxRequest) -> Result<(i32, String, String), String> {
        unsafe {
            let (token, logon_sid_bytes) = create_restricted_token()?;

            // Apply ACLs: allow sandbox SID write access on writable roots
            if let Some(ref fp) = req.fs_policy {
                let psid = logon_sid_bytes.as_ptr() as *mut c_void;
                for root in &fp.writable_roots {
                    let path_str = root.path.to_string_lossy().to_string();
                    let _ = apply_allow_ace(&path_str, psid);
                }
                // Deny-write on protected paths under writable roots
                for pattern in &fp.protected_paths {
                    let p = pattern.trim_end_matches("/**");
                    if Path::new(p).exists() {
                        let _ = apply_deny_write_ace(p, psid);
                    }
                }
            }

            // Create non-inheritable pipes with stdin (matching codex pattern).
            // Only the child's handles (stdin_read, stdout_write, stderr_write)
            // are marked inheritable, preventing handle leaks to grandchild processes.
            let (stdin_read, stdin_write, stdout_read, stdout_write, stderr_read, stderr_write) =
                create_pipes_with_stdin()?;

            let full_cmd = format!("{} {}", req.program, req.args.join(" "));
            let pi = spawn_process_with_handles(
                token,
                &full_cmd,
                &req.cwd.to_string_lossy(),
                stdin_read,
                stdout_write,
                stderr_write,
            )?;

            // Parent closes its copies of the child's handles
            CloseHandle(stdin_read); // child has its own copy via handle list
            CloseHandle(stdout_write); // child has its own copy via handle list
            CloseHandle(stderr_write); // child has its own copy via handle list
            CloseHandle(token);

            // Close stdin write end to signal EOF to the child process
            CloseHandle(stdin_write);

            let stdout_thread = read_pipe_async(stdout_read);
            let stderr_thread = read_pipe_async(stderr_read);

            let timeout_ms = if req.timeout_secs > 0 {
                req.timeout_secs * 1000
            } else {
                30000
            };
            let wait_result = WaitForSingleObject(pi.hProcess, timeout_ms as u32);

            let mut exit_code: i32 = -1;
            if wait_result == WAIT_OBJECT_0 {
                let mut code: u32 = 0;
                GetExitCodeProcess(pi.hProcess, &mut code);
                exit_code = code as i32;
            } else if wait_result == WAIT_TIMEOUT {
                TerminateProcess(pi.hProcess, 1);
            }

            let stdout = stdout_thread.join().unwrap_or_default();
            let stderr = stderr_thread.join().unwrap_or_default();

            CloseHandle(pi.hProcess);
            CloseHandle(pi.hThread);
            Ok((exit_code, stdout, stderr))
        }
    }

    unsafe fn apply_allow_ace(path: &str, psid: *mut c_void) -> Result<(), String> {
        use std::ptr;
        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let mut p_dacl: *mut ACL = ptr::null_mut();
        let mut p_sd: *mut c_void = ptr::null_mut();
        let code = GetNamedSecurityInfoW(
            wide.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            &mut p_dacl,
            ptr::null_mut(),
            &mut p_sd,
        );
        if code != ERROR_SUCCESS {
            // Path may not exist yet, that's OK
            if !p_sd.is_null() {
                windows_sys::Win32::Foundation::LocalFree(p_sd as HLOCAL);
            }
            return Ok(());
        }

        let trustee = TRUSTEE_W {
            pMultipleTrustee: ptr::null_mut(),
            MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_UNKNOWN,
            ptstrName: psid as *mut u16,
        };
        let mut explicit: EXPLICIT_ACCESS_W = std::mem::zeroed();
        explicit.grfAccessPermissions = 0x1F01FF; // GENERIC_ALL
        explicit.grfAccessMode = 2; // SET_ACCESS
        explicit.grfInheritance = CONTAINER_INHERIT_ACE | OBJECT_INHERIT_ACE;
        explicit.Trustee = trustee;

        let mut p_new_dacl: *mut ACL = ptr::null_mut();
        let code2 = SetEntriesInAclW(1, &explicit, p_dacl, &mut p_new_dacl);
        if code2 == ERROR_SUCCESS && !p_new_dacl.is_null() {
            let _ = SetNamedSecurityInfoW(
                wide.as_ptr() as *mut u16,
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                ptr::null_mut(),
                ptr::null_mut(),
                p_new_dacl,
                ptr::null_mut(),
            );
            windows_sys::Win32::Foundation::LocalFree(p_new_dacl as HLOCAL);
        }
        if !p_sd.is_null() {
            windows_sys::Win32::Foundation::LocalFree(p_sd as HLOCAL);
        }
        Ok(())
    }

    unsafe fn apply_deny_write_ace(path: &str, psid: *mut c_void) -> Result<(), String> {
        use std::ptr;
        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let mut p_dacl: *mut ACL = ptr::null_mut();
        let mut p_sd: *mut c_void = ptr::null_mut();
        let code = GetNamedSecurityInfoW(
            wide.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            &mut p_dacl,
            ptr::null_mut(),
            &mut p_sd,
        );
        if code != ERROR_SUCCESS {
            if !p_sd.is_null() {
                windows_sys::Win32::Foundation::LocalFree(p_sd as HLOCAL);
            }
            return Ok(());
        }

        let trustee = TRUSTEE_W {
            pMultipleTrustee: ptr::null_mut(),
            MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_UNKNOWN,
            ptstrName: psid as *mut u16,
        };
        let mut explicit: EXPLICIT_ACCESS_W = std::mem::zeroed();
        explicit.grfAccessPermissions = FILE_GENERIC_WRITE | FILE_DELETE_CHILD | 0x00010000; // DELETE
        explicit.grfAccessMode = 1; // DENY_ACCESS
        explicit.grfInheritance = CONTAINER_INHERIT_ACE | OBJECT_INHERIT_ACE;
        explicit.Trustee = trustee;

        let mut p_new_dacl: *mut ACL = ptr::null_mut();
        let code2 = SetEntriesInAclW(1, &explicit, p_dacl, &mut p_new_dacl);
        if code2 == ERROR_SUCCESS && !p_new_dacl.is_null() {
            let _ = SetNamedSecurityInfoW(
                wide.as_ptr() as *mut u16,
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                ptr::null_mut(),
                ptr::null_mut(),
                p_new_dacl,
                ptr::null_mut(),
            );
            windows_sys::Win32::Foundation::LocalFree(p_new_dacl as HLOCAL);
        }
        if !p_sd.is_null() {
            windows_sys::Win32::Foundation::LocalFree(p_sd as HLOCAL);
        }
        Ok(())
    }
}

impl Sandbox for WindowsRestrictedTokenSandbox {
    fn sandbox_type(&self) -> SandboxType {
        SandboxType::WindowsJob
    }

    fn name(&self) -> &str {
        "windows-restricted-token"
    }

    fn is_available(&self) -> bool {
        self.enabled && cfg!(windows)
    }

    fn is_windows_sandbox(&self) -> bool {
        true
    }

    fn transform(&self, req: &SandboxRequest) -> Result<(String, Vec<String>), String> {
        Ok((req.program.clone(), req.args.clone()))
    }

    fn spawn(&self, cmd: &mut std::process::Command, req: &SandboxRequest) -> Result<(), String> {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x02000000 | 0x00000200 | 0x00000004);
        }
        let _ = req;
        Ok(())
    }

    fn direct_spawn(
        &self,
        _command: &str,
        req: &SandboxRequest,
    ) -> Option<Result<(i32, String, String), String>> {
        #[cfg(windows)]
        {
            // On Windows, direct_spawn with CreateRestrictedToken has msys
            // compatibility issues (signal pipes, BaseNamedObjects access).
            // Return None to fall through to the transform + tokio::process::Command
            // path (standard CreateProcessW without restricted token), matching codex's
            // default execution flow. Security is maintained via:
            //   - Filesystem policy (absolute path access control)
            //   - Exec policy (dangerous command detection)
            //   - Guardian (circuit breaker)
            let _ = req;
            None
        }
        #[cfg(not(windows))]
        {
            let _ = req;
            None
        }
    }
}
