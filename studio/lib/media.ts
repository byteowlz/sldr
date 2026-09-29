// Naming for media added from the studio (paste / drop). Clipboard images
// arrive as a generic "image.png", so those get a name derived from the
// slide; a real file keeps its own name. The server refuses to overwrite,
// so a collision just retries with a suffix.

const EXT: Record<string, string> = {
  "image/png": "png",
  "image/jpeg": "jpg",
  "image/gif": "gif",
  "image/webp": "webp",
  "image/svg+xml": "svg",
  "video/mp4": "mp4",
  "video/webm": "webm",
};

export const isMediaFile = (f: File) => f.type in EXT || /\.(png|jpe?g|gif|webp|svg|mp4|webm)$/i.test(f.name);

export function mediaNameFor(slide: string, file: File, attempt = 0): string {
  const stem = slide.replace(/\.md$/, "").split("/").pop() ?? "slide";
  const generic = !file.name || /^image\.(png|jpe?g|gif|webp)$/i.test(file.name);
  const ext = EXT[file.type] ?? file.name.split(".").pop()?.toLowerCase() ?? "png";
  const base = generic
    ? `${stem}-${new Date().toISOString().slice(0, 19).replace(/[-:T]/g, "")}`
    : file.name.replace(/\.[^.]+$/, "").replace(/[^\w.-]+/g, "-");
  return `${base}${attempt ? `-${attempt + 1}` : ""}.${ext}`;
}

/** The markdown for a stored file: an image, or an inline video tag. */
export const markdownFor = (reference: string, alt = "") =>
  /\.(mp4|webm)$/i.test(reference) ? `<video src="${reference}" controls></video>` : `![${alt}](${reference})`;
