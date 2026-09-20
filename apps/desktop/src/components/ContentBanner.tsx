import { useState, type CSSProperties } from "react";
import { cn } from "../lib/cn";

export function ContentBanner({
  bannerSrc,
  iconSrc,
  name,
  className,
  imageClassName,
  imageStyle,
  eager = false,
}: {
  bannerSrc?: string | null;
  iconSrc?: string | null;
  name: string;
  className?: string;
  imageClassName?: string;
  imageStyle?: CSSProperties;
  eager?: boolean;
}) {
  const [failedBanner, setFailedBanner] = useState<string>();
  const [failedIcon, setFailedIcon] = useState<string>();
  const banner =
    bannerSrc && bannerSrc !== failedBanner ? bannerSrc : undefined;
  const icon = iconSrc && iconSrc !== failedIcon ? iconSrc : undefined;

  return (
    <span
      className={cn(
        "relative inline-flex shrink-0 overflow-hidden bg-app-raised",
        className,
      )}
      title={name}
      aria-hidden="true"
    >
      {banner ? (
        <img
          src={banner}
          alt=""
          className={cn("size-full object-cover", imageClassName)}
          style={imageStyle}
          loading={eager ? "eager" : "lazy"}
          decoding="async"
          referrerPolicy="no-referrer"
          onError={() => setFailedBanner(banner)}
        />
      ) : icon ? (
        <>
          <img
            src={icon}
            alt=""
            className="absolute -inset-[12%] size-[124%] scale-110 object-cover opacity-45 blur-[52px]"
            loading={eager ? "eager" : "lazy"}
            decoding="async"
            referrerPolicy="no-referrer"
            onError={() => setFailedIcon(icon)}
          />
          <span className="absolute inset-0 bg-app-bg/20" aria-hidden="true" />
        </>
      ) : (
        <span className="absolute inset-0 bg-app-raised" aria-hidden="true" />
      )}
    </span>
  );
}
