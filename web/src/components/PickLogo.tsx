export function PickLogo() {
  return (
    <div className="flex flex-col items-center gap-2 select-none">
      <svg
        width="296"
        height="96"
        viewBox="0 0 296 96"
        xmlns="http://www.w3.org/2000/svg"
        className="text-neutral-100"
        style={{ fontFamily: "var(--font-family-sans)" }}
      >
        <text
          x="0"
          y="72"
          fill="var(--accent-primary)"
          fontSize="100"
          fontWeight="700"
        >
          P
        </text>
        <text
          x="66"
          y="72"
          fill="currentColor"
          fontSize="80"
          fontWeight="700"
        >
          ick
        </text>
      </svg>
      <span className="text-xs text-neutral-500 tracking-wider">
        AI-powered coding assistant
      </span>
    </div>
  );
}
