import { useState } from "react";
import { cn } from "../lib/cn";

const fallbackArtwork = [
  "/artwork/alpine-lake.webp",
  "/artwork/verdant-workshop.webp",
  "/artwork/ember-citadel.webp",
] as const;

function fallbackArtworkFor(stableKey: string) {
  let hash = 2166136261;
  for (const character of stableKey) {
    hash ^= character.codePointAt(0) ?? 0;
    hash = Math.imul(hash, 16777619);
  }
  return fallbackArtwork[(hash >>> 0) % fallbackArtwork.length];
}

export function ContentArtwork({
  src,
  name,
  stableKey = name,
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
  const source = remoteSource ?? fallbackArtworkFor(stableKey);

  return (
    <span
      className={cn(
        "relative inline-flex shrink-0 overflow-hidden bg-app-raised",
        className,
      )}
      title={name}
    >
      <img
        src={source}
        alt=""
        className={cn("size-full object-cover", imageClassName)}
        loading={eager ? "eager" : "lazy"}
        decoding="async"
        referrerPolicy="no-referrer"
        onError={() => {
          if (remoteSource) setFailedSource(remoteSource);
        }}
      />
    </span>
  );
}
