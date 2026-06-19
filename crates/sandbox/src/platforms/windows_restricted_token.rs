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
const SE_GROUP_LOGON_ID: u32 = 0x0000_0002;
const DACL_SECURITY_INFORMATION: u32 = 4;
const CONTAINER_INHERIT_ACE: u32 = 2;
const OBJECT_INHERIT_ACE: u32 = 1;
const HANDLE_FLAG_INHERIT: u32 = 1;

#[cfg(windows)]
pub(crate) mod win_impl {
    use std::ffi::c_void;
    use std::path::Path;
    use std::ptr;

    use super::{CONTAINER_INHERIT_ACE, DACL_SECURITY_INFORMATION, OBJECT_INHERIT_ACE};
    use pick_agent::permission::sandbox::SandboxRequest;

    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_SUCCESS, GetLastError, HANDLE, HLOCAL, INVALID_HANDLE_VALUE,
        WAIT_OBJECT_0, WAIT_TIMEOUT,
    };
    use windows_sys::Win32::Security::Authorization::{
        EXPLICIT_ACCESS_W, GetNamedSecurityInfoW, NO_MULTIPLE_TRUSTEE, SE_FILE_OBJECT,
        SetEntriesInAclW, SetNamedSecurityInfoW, TRUSTEE_IS_SID, TRUSTEE_IS_USER, TRUSTEE_W,
    };
    use windows_sys::Win32::Security::{
        ACL, CreateRestrictedToken, DISABLE_MAX_PRIVILEGE, GetLengthSid, GetTokenInformation,
        LUA_TOKEN, SID_AND_ATTRIBUTES, SetTokenInformation, TOKEN_ADJUST_DEFAULT,
        TOKEN_ADJUST_PRIVILEGES, TOKEN_ADJUST_SESSIONID, TOKEN_ASSIGN_PRIMARY, TOKEN_DUPLICATE,
        TOKEN_GROUPS, TOKEN_QUERY, TokenDefaultDacl, TokenGroups, WRITE_RESTRICTED,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_DELETE_CHILD, FILE_GENERIC_WRITE, ReadFile,
    };
    use windows_sys::Win32::System::Pipes::CreatePipe;
    use windows_sys::Win32::System::Threading::{
        CreateProcessAsUserW, GetCurrentProcess, GetExitCodeProcess, OpenProcessToken,
        PROCESS_INFORMATION, STARTUPINFOW, TerminateProcess, WaitForSingleObject,
    };

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

        let logon_sid_bytes = get_restricting_sid_bytes(h_token)?;
        let psid_logon = logon_sid_bytes.as_ptr() as *mut c_void;

        let mut entries: Vec<SID_AND_ATTRIBUTES> = vec![std::mem::zeroed()];
        entries[0].Sid = psid_logon;

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

        // Set a permissive default DACL so the process can create pipes
        let trustee = TRUSTEE_W {
            pMultipleTrustee: ptr::null_mut(),
            MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_USER,
            ptstrName: psid_logon as *mut u16,
        };
        let explicit = EXPLICIT_ACCESS_W {
            grfAccessPermissions: 0x1F01FF,
            grfAccessMode: 2,
            grfInheritance: 0,
            Trustee: trustee,
        };
        let mut p_new_dacl: *mut ACL = ptr::null_mut();
        let code = SetEntriesInAclW(1, &explicit, ptr::null(), &mut p_new_dacl);
        if code == ERROR_SUCCESS && !p_new_dacl.is_null() {
            let _ = SetTokenInformation(
                new_token,
                TokenDefaultDacl,
                &p_new_dacl as *const *mut ACL as *const c_void,
                std::mem::size_of::<*mut ACL>() as u32,
            );
            windows_sys::Win32::Foundation::LocalFree(p_new_dacl as HLOCAL);
        }

        Ok((new_token, logon_sid_bytes))
    }

    unsafe fn get_restricting_sid_bytes(token: HANDLE) -> Result<Vec<u8>, String> {
        let mut size: u32 = 0;
        let _ = GetTokenInformation(token, TokenGroups, ptr::null_mut(), 0, &mut size);
        if size == 0 {
            return Ok(vec![0u8; 68]);
        }
        let mut buf = vec![0u8; size as usize];
        let ok = GetTokenInformation(
            token,
            TokenGroups,
            buf.as_mut_ptr() as *mut c_void,
            size,
            &mut size,
        );
        if ok == 0 {
            return Ok(vec![0u8; 68]);
        }
        let groups = buf.as_ptr() as *const TOKEN_GROUPS;
        let count = (*groups).GroupCount as usize;
        for i in 0..count {
            let attr = (*groups).Groups.as_ptr().add(i).read().Attributes;
            if attr & super::SE_GROUP_LOGON_ID != 0 {
                let sid_ptr = (*groups).Groups.as_ptr().add(i).read().Sid;
                let sid_len = GetLengthSid(sid_ptr) as usize;
                let mut sid_bytes = vec![0u8; sid_len];
                std::ptr::copy_nonoverlapping(
                    sid_ptr as *const u8,
                    sid_bytes.as_mut_ptr(),
                    sid_len,
                );
                return Ok(sid_bytes);
            }
        }
        Ok(vec![0u8; 68])
    }

    unsafe fn create_pipes() -> Result<(HANDLE, HANDLE, HANDLE, HANDLE), String> {
        let mut stdout_read: HANDLE = 0;
        let mut stdout_write: HANDLE = 0;
        let mut stderr_read: HANDLE = 0;
        let mut stderr_write: HANDLE = 0;

        let mut sa: windows_sys::Win32::Security::SECURITY_ATTRIBUTES = std::mem::zeroed();
        sa.nLength =
            std::mem::size_of::<windows_sys::Win32::Security::SECURITY_ATTRIBUTES>() as u32;
        sa.bInheritHandle = 1;

        if CreatePipe(&mut stdout_read, &mut stdout_write, &sa, 0) == 0 {
            return Err(format!("CreatePipe stdout failed: {}", GetLastError()));
        }
        if CreatePipe(&mut stderr_read, &mut stderr_write, &sa, 0) == 0 {
            CloseHandle(stdout_read);
            CloseHandle(stdout_write);
            return Err(format!("CreatePipe stderr failed: {}", GetLastError()));
        }

        // The parent closes its copy of the write end, so the child gets EOF
        // when done writing. Read ends remain inheritable which is fine for
        // this use case (the parent reads from them in dedicated threads).
        Ok((stdout_read, stdout_write, stderr_read, stderr_write))
    }

    unsafe fn spawn_process(
        token: HANDLE,
        command_line: &str,
        cwd: &str,
        stdout_write: HANDLE,
        stderr_write: HANDLE,
    ) -> Result<PROCESS_INFORMATION, String> {
        let mut cmd_ws: Vec<u16> = command_line
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let cwd_ws: Vec<u16> = cwd.encode_utf16().chain(std::iter::once(0)).collect();

        let si = STARTUPINFOW {
            cb: std::mem::size_of::<STARTUPINFOW>() as u32,
            dwFlags: 0x00000100 | 0x00000001,
            wShowWindow: 0,
            hStdInput: INVALID_HANDLE_VALUE,
            hStdOutput: stdout_write,
            hStdError: stderr_write,
            ..std::mem::zeroed()
        };
        let mut pi = PROCESS_INFORMATION {
            hProcess: 0,
            hThread: 0,
            dwProcessId: 0,
            dwThreadId: 0,
        };

        let flags = 0x02000000 | 0x00000004; // CREATE_BREAKAWAY_FROM_JOB | CREATE_NEW_CONSOLE
        let ok = CreateProcessAsUserW(
            token,
            ptr::null(),
            cmd_ws.as_mut_ptr(),
            ptr::null(),
            ptr::null(),
            1,
            flags,
            ptr::null(),
            cwd_ws.as_ptr(),
            &si,
            &mut pi,
        );
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

            let (stdout_read, stdout_write, stderr_read, stderr_write) = create_pipes()?;

            let full_cmd = format!("{} {}", req.program, req.args.join(" "));
            let pi = spawn_process(
                token,
                &full_cmd,
                &req.cwd.to_string_lossy(),
                stdout_write,
                stderr_write,
            )?;

            CloseHandle(stdout_write);
            CloseHandle(stderr_write);
            CloseHandle(token);

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
            TrusteeType: TRUSTEE_IS_USER,
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
            TrusteeType: TRUSTEE_IS_USER,
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
            Some(win_impl::run_sandboxed_command(req))
        }
        #[cfg(not(windows))]
        {
            let _ = req;
            None
        }
    }
}
