/**
 * @typedef {Object} FileRecord
 * @property {string} id
 * @property {string} name
 * @property {number} total_size
 * @property {string} mime_type
 * @property {string} created_at
 * @property {string|null} folder_id
 */

/**
 * @typedef {Object} FolderRecord
 * @property {string} id
 * @property {string} name
 * @property {string|null} parent_id
 * @property {string} created_at
 */

/**
 * @typedef {Object} Breadcrumb
 * @property {string|null} id
 * @property {string} name
 */

export {}; // Ensure this file is treated as a module
