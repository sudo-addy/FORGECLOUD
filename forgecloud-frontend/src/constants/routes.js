export const API_ROUTES = {
  FILES: "/v1/files",
  FILES_UPLOAD: "/v1/files/upload",
  FILES_DOWNLOAD: (id) => `/v1/files/download/${id}`,
  FILES_UPDATE: (id) => `/v1/files/${id}`,
  FILES_DELETE: (id) => `/v1/files/${id}`,
  FOLDERS: "/v1/folders",
  SHARES_CREATE: (id) => `/v1/files/${id}/shares`,
  SHARES_LIST: (id) => `/v1/files/${id}/shares`,
  SHARES_DELETE: (id) => `/v1/shares/${id}`,
  SHARES_PUBLIC_INFO: (token) => `/v1/shares/public/${token}`,
  SHARES_PUBLIC_DOWNLOAD: (token) => `/v1/shares/public/${token}/download`,
};
