import { useQuery } from "@tanstack/react-query";
import { getInstanceArtwork } from "../lib/bridge";
import type { LauncherInstance } from "../types/launcher";
import { ContentArtwork } from "./ContentArtwork";
import { ContentBanner } from "./ContentBanner";

export function InstanceArtwork({
  instance,
  fallbackSrc,
  className,
  eager = false,
}: {
  instance: LauncherInstance;
  fallbackSrc?: string | null;
  className?: string;
  eager?: boolean;
}) {
  const query = useQuery({
    queryKey: ["instance-artwork", instance.id, "icon", instance.revision],
    queryFn: () => getInstanceArtwork(instance.id, "icon"),
    enabled: instance.settings.hasCustomIcon,
    staleTime: Number.POSITIVE_INFINITY,
  });
  return (
    <ContentArtwork
      src={query.data ?? instance.modpackSource?.iconUrl ?? fallbackSrc}
      name={instance.name}
      className={className}
      eager={eager}
    />
  );
}

export function InstanceBanner({
  instance,
  fallbackBanner,
  fallbackIcon,
  className,
  eager = false,
}: {
  instance: LauncherInstance;
  fallbackBanner?: string | null;
  fallbackIcon?: string | null;
  className?: string;
  eager?: boolean;
}) {
  const query = useQuery({
    queryKey: ["instance-artwork", instance.id, "banner", instance.revision],
    queryFn: () => getInstanceArtwork(instance.id, "banner"),
    enabled: instance.settings.hasCustomBanner,
    staleTime: Number.POSITIVE_INFINITY,
  });
  return (
    <ContentBanner
      bannerSrc={query.data ?? fallbackBanner}
      iconSrc={fallbackIcon ?? instance.modpackSource?.iconUrl}
      name={`${instance.name} banner`}
      className={className}
      imageStyle={{
        objectPosition: `${instance.settings.bannerPositionX}% ${instance.settings.bannerPositionY}%`,
      }}
      eager={eager}
    />
  );
}
