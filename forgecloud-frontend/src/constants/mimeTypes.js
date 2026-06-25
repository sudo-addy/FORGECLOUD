export const PREVIEWABLE_MIME_PREFIXES = {
  IMAGE: "image/",
  VIDEO: "video/",
  AUDIO: "audio/",
  TEXT: "text/",
};

export const PREVIEWABLE_MIME_EXACT = {
  PDF: "application/pdf",
  JSON: "application/json",
};

export const isPreviewable = (mime) => {
  if (!mime) return false;
  
  return (
    mime.startsWith(PREVIEWABLE_MIME_PREFIXES.IMAGE) ||
    mime.startsWith(PREVIEWABLE_MIME_PREFIXES.VIDEO) ||
    mime.startsWith(PREVIEWABLE_MIME_PREFIXES.AUDIO) ||
    mime.startsWith(PREVIEWABLE_MIME_PREFIXES.TEXT) ||
    mime === PREVIEWABLE_MIME_EXACT.PDF ||
    mime === PREVIEWABLE_MIME_EXACT.JSON
  );
};
