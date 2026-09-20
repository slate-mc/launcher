import ReactMarkdown, { defaultUrlTransform } from "react-markdown";
import rehypeRaw from "rehype-raw";
import rehypeSanitize from "rehype-sanitize";
import remarkGfm from "remark-gfm";
import { cn } from "../lib/cn";

export function ProviderMarkdown({
  value,
  className,
}: {
  value: string;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "provider-markdown text-[13px]/[21px] text-app-secondary",
        className,
      )}
    >
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[rehypeRaw, rehypeSanitize]}
        urlTransform={defaultUrlTransform}
        components={{
          h1: ({ children }) => (
            <h3 className="mt-6 mb-2 text-lg font-bold text-app-text first:mt-0">
              {children}
            </h3>
          ),
          h2: ({ children }) => (
            <h3 className="mt-6 mb-2 text-base font-bold text-app-text first:mt-0">
              {children}
            </h3>
          ),
          h3: ({ children }) => (
            <h3 className="mt-5 mb-2 text-sm font-bold text-app-text first:mt-0">
              {children}
            </h3>
          ),
          p: ({ children }) => (
            <p className="my-3 first:mt-0 last:mb-0">{children}</p>
          ),
          ul: ({ children }) => (
            <ul className="my-3 list-disc space-y-1 pl-5">{children}</ul>
          ),
          ol: ({ children }) => (
            <ol className="my-3 list-decimal space-y-1 pl-5">{children}</ol>
          ),
          blockquote: ({ children }) => (
            <blockquote className="my-4 border-l-2 border-app-accent/60 bg-app-surface px-4 py-2 text-app-secondary">
              {children}
            </blockquote>
          ),
          a: ({ href, children }) => (
            <a
              href={href}
              target="_blank"
              rel="noreferrer noopener"
              className="font-semibold text-app-accent underline decoration-app-accent/40 underline-offset-2 hover:decoration-app-accent"
            >
              {children}
            </a>
          ),
          img: ({ src, alt }) =>
            typeof src === "string" ? (
              <img
                src={src}
                alt={alt ?? ""}
                loading="lazy"
                decoding="async"
                referrerPolicy="no-referrer"
                className="my-4 max-h-[420px] max-w-full rounded-control border border-app-separator object-contain"
              />
            ) : null,
          code: ({ children }) => (
            <code className="rounded-compact bg-app-raised px-1.5 py-0.5 font-mono text-[11px] text-app-text">
              {children}
            </code>
          ),
          pre: ({ children }) => (
            <pre className="my-4 overflow-x-auto rounded-control border border-app-separator bg-app-sidebar p-4 font-mono text-[11px]/[18px] text-app-text">
              {children}
            </pre>
          ),
          table: ({ children }) => (
            <div className="my-4 overflow-x-auto">
              <table className="w-full border-collapse text-left text-xs">
                {children}
              </table>
            </div>
          ),
          th: ({ children }) => (
            <th className="border-b border-app-separator bg-app-surface px-3 py-2 text-app-text">
              {children}
            </th>
          ),
          td: ({ children }) => (
            <td className="border-b border-app-separator/50 px-3 py-2">
              {children}
            </td>
          ),
        }}
      >
        {value.slice(0, 50_000)}
      </ReactMarkdown>
    </div>
  );
}
