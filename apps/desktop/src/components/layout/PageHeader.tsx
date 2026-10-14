import type { ReactNode } from "react";

export function PageHeader({ title, actions }: { title: string; actions?: ReactNode }) {
  return (
    <header className="flex items-center justify-between gap-4 px-8 pb-4 pt-7">
      <h1 className="text-xl font-semibold">{title}</h1>
      {actions}
    </header>
  );
}
