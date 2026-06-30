import type { ButtonHTMLAttributes, InputHTMLAttributes, ReactNode } from "react";
import { cn } from "@/lib/utils";

export function Button({
  className,
  variant = "default",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "default" | "ghost" | "outline";
}) {
  return (
    <button
      className={cn(
        "inline-flex items-center justify-center gap-2 rounded-md px-3 py-2 text-sm font-medium transition-colors disabled:opacity-50 disabled:pointer-events-none",
        variant === "default" &&
          "bg-[var(--color-primary)] text-[var(--color-primary-fg)] hover:opacity-90",
        variant === "outline" &&
          "border border-[var(--color-border)] hover:bg-[var(--color-card)]",
        variant === "ghost" && "hover:bg-[var(--color-card)]",
        className,
      )}
      {...props}
    />
  );
}

export function Input({
  className,
  ...props
}: InputHTMLAttributes<HTMLInputElement>) {
  return (
    <input
      className={cn(
        "w-full rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2 text-sm outline-none focus:border-[var(--color-primary)]",
        className,
      )}
      {...props}
    />
  );
}

export function Card({
  className,
  children,
}: {
  className?: string;
  children: ReactNode;
}) {
  return (
    <div
      className={cn(
        "rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] p-4",
        className,
      )}
    >
      {children}
    </div>
  );
}

export function Muted({ children }: { children: ReactNode }) {
  return <span className="text-[var(--color-muted)]">{children}</span>;
}

export function Spinner() {
  return (
    <div className="animate-spin h-4 w-4 rounded-full border-2 border-[var(--color-border)] border-t-[var(--color-primary)]" />
  );
}
