/**
 * Formats bytes into a human-readable string.
 * @param {number} bytes 
 * @returns {string} Formatted string
 */
export const formatBytes = (bytes) => {
  if (bytes === 0) return "0 Bytes";
  return (bytes / 1024).toFixed(2) + " KB";
};

/**
 * Formats a date string into a local locale string.
 * @param {string} dateString 
 * @returns {string} Formatted string
 */
export const formatDate = (dateString) => {
  if (!dateString) return "";
  return new Date(dateString).toLocaleString();
};

/**
 * Parses the filename from a Content-Disposition header.
 * @param {string} contentDisposition 
 * @param {string} fallback 
 * @returns {string} The extracted filename
 */
export const extractFilenameFromHeader = (contentDisposition, fallback = "downloaded_file.bin") => {
  if (contentDisposition && contentDisposition.indexOf("filename=") !== -1) {
    const filenameMatch = contentDisposition.match(/filename="?([^"]+)"?/);
    if (filenameMatch && filenameMatch.length === 2) {
      return filenameMatch[1];
    }
  }
  return fallback;
};
