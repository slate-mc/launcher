import { useState } from "react";
import { cn } from "../lib/cn";

export function MinecraftHead({
  skinUrl,
  playerName,
  className,
}: {
  skinUrl?: string;
  playerName: string;
  className?: string;
}) {
  const [failedUrl, setFailedUrl] = useState<string>();
  const showSkin = Boolean(skinUrl) && failedUrl !== skinUrl;
  const imageClass =
    "pointer-events-none absolute h-auto w-[800%] max-w-none select-none [image-rendering:pixelated]";

  return (
    <span
      className={cn(
        "relative inline-flex shrink-0 items-center justify-center overflow-hidden rounded-compact bg-app-accent font-extrabold text-app-on-accent",
        className,
      )}
      role="img"
      aria-label={`${playerName}'s Minecraft skin`}
    >
      {showSkin ? (
        <>
          <img
            src={skinUrl}
            alt=""
            draggable={false}
            className={cn(imageClass, "top-[-100%] left-[-100%]")}
            onError={() => setFailedUrl(skinUrl)}
          />
          <img
            src={skinUrl}
            alt=""
            draggable={false}
            className={cn(imageClass, "top-[-100%] left-[-500%]")}
            onError={() => setFailedUrl(skinUrl)}
          />
        </>
      ) : (
        playerName.slice(0, 1).toUpperCase()
      )}
    </span>
  );
}
