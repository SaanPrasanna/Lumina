import type { ButtonHTMLAttributes, ReactNode } from "react";

/** Toolbar / card action button with an icon. */
export function Button({
  children,
  icon,
  primary,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { icon?: ReactNode; primary?: boolean }) {
  return (
    <button
      type="button"
      {...props}
      className={`flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-sm font-medium outline-none transition-[background-color,border-color,color,transform] duration-150 ease-out focus-visible:ring-3 focus-visible:ring-accent/50 active:not-disabled:translate-y-px disabled:opacity-50 [&_svg]:size-4 ${
        primary ? "bg-accent text-on-accent hover:bg-accent-strong" : "border border-line hover:bg-hover"
      }`}
    >
      {icon}
      {children}
    </button>
  );
}

/** A labelled value in a card's facts grid. */
export function Fact({ label, value }: { label: string; value: ReactNode }) {
  return (
    <div className="min-w-0">
      <dt className="truncate text-xs text-muted">{label}</dt>
      <dd className="mt-0.5 font-mono text-sm" dir="ltr">
        {value}
      </dd>
    </div>
  );
}

/** Version picker: "Newest installed (x)" plus every installed version. */
export function VersionSelect(props: {
  label: string;
  newestLabel: string;
  versions: string[];
  value: string;
  disabled?: boolean;
  onChange: (version: string) => void;
}) {
  return (
    <label className="flex min-w-0 flex-1 flex-col gap-1.5 text-sm">
      <span className="font-medium">{props.label}</span>
      <select
        className="rounded-md border border-line bg-surface px-3 py-2"
        value={props.value}
        disabled={props.disabled}
        onChange={(e) => props.onChange(e.target.value)}
      >
        <option value="">{props.newestLabel}</option>
        {props.versions.map((v) => (
          <option key={v} value={v}>
            {v}
          </option>
        ))}
      </select>
    </label>
  );
}
