import { ArrowLeft } from "lucide-react";
import { Link } from "@tanstack/react-router";

type RoutePlaceholderProps = {
  eyebrow: string;
  title: string;
  description: string;
};

export function RoutePlaceholder({
  eyebrow,
  title,
  description,
}: RoutePlaceholderProps) {
  return (
    <section className="mx-auto flex min-h-full max-w-[620px] flex-col items-start justify-center px-8 py-[10vh]">
      <p className="mb-2 text-xs/[16px] font-bold tracking-[.09em] text-app-secondary uppercase">
        {eyebrow}
      </p>
      <h1 className="m-0 text-[34px]/[40px] font-bold tracking-[-.035em] text-app-text">
        {title}
      </h1>
      <p className="mt-3 mb-6 max-w-[560px] text-sm text-app-secondary">
        {description}
      </p>
      <Link
        className="inline-flex h-[34px] items-center justify-center gap-2 rounded-[7px] border border-app-separator bg-app-raised px-[18px] text-[11px] font-bold text-app-text no-underline transition-colors duration-[120ms] hover:bg-app-hover"
        to="/home"
      >
        <ArrowLeft size={17} aria-hidden="true" />
        Back home
      </Link>
    </section>
  );
}
