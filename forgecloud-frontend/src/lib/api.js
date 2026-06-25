import { CONFIG } from "./config";

/**
 * Generates headers for authenticated requests
 * @returns {Record<string, string>} Headers object containing the API key if present
 */
export const getAuthHeaders = () => {
  const storedKey = localStorage.getItem(CONFIG.API_KEY_STORAGE_KEY);
  const headers = {};
  if (storedKey) {
    headers["x-api-key"] = storedKey;
  }
  return headers;
};
