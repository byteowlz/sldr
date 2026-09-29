// Previews as data: a section asks the backend for a Preview (URL or HTML)
// through react-query, and SlideFrame renders either. This is the seam that
// lets the hosted app show renderer output without a network.

import { useQuery } from "@tanstack/react-query";
import { useBackend } from "./index";
import type { Preview } from "./types";

export function useSlidePreview(slide: string | null, opts?: { flavor?: string; layout?: string; bust?: number }) {
  const b = useBackend();
  return useQuery<Preview>({
    queryKey: ["preview", "slide", b.id, slide, opts?.flavor ?? null, opts?.layout ?? null, opts?.bust ?? 0],
    queryFn: () => b.previewSlide(slide!, opts),
    enabled: !!slide,
    staleTime: Infinity,
  });
}

export function useLayoutPreview(layout: string | null, flavor?: string | null) {
  const b = useBackend();
  return useQuery<Preview>({
    queryKey: ["preview", "layout", b.id, layout, flavor ?? null],
    queryFn: () => b.previewLayout(layout!, flavor ?? undefined),
    enabled: !!layout,
    staleTime: Infinity,
  });
}

export function useSamplePreview(flavor: string | null, bust?: number) {
  const b = useBackend();
  return useQuery<Preview>({
    queryKey: ["preview", "sample", b.id, flavor, bust ?? 0],
    queryFn: () => b.previewSample(flavor!, bust),
    enabled: !!flavor,
    staleTime: Infinity,
  });
}
