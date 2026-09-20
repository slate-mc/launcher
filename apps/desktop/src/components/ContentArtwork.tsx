import { useState } from "react";
import { cn } from "../lib/cn";

export function ContentArtwork({
  src,
  name,
  className,
  imageClassName,
  eager = false,
}: {
  src?: string | null;
  name: string;
  stableKey?: string;
  className?: string;
  imageClassName?: string;
  eager?: boolean;
}) {
  const [failedSource, setFailedSource] = useState<string>();
  const remoteSource = src && src !== failedSource ? src : undefined;

  return (
    <span
      className={cn(
        "relative inline-flex shrink-0 overflow-hidden bg-app-raised",
        className,
      )}
      title={name}
    >
      {remoteSource ? (
        <img
          src={remoteSource}
          alt=""
          className={cn("size-full object-cover", imageClassName)}
          loading={eager ? "eager" : "lazy"}
          decoding="async"
          referrerPolicy="no-referrer"
          onError={() => setFailedSource(remoteSource)}
        />
      ) : (
        <img
          src="/brand/slate-symbol-jade.svg"
          alt=""
          className={cn(
            "size-full object-contain p-[24%] opacity-55",
            imageClassName,
          )}
          loading={eager ? "eager" : "lazy"}
          decoding="async"
        />
      )}
    </span>
  );
}
