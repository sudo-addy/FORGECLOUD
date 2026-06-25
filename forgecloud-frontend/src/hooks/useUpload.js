import { useState } from "react";
import { CONFIG } from "../lib/config";
import { API_ROUTES } from "../constants/routes";

export function useUpload(currentFolderId, fetchVaultFiles) {
  const [selectedFile, setSelectedFile] = useState(null);
  const [isUploading, setIsUploading] = useState(false);
  const [uploadStatus, setUploadStatus] = useState("");
  const [uploadProgress, setUploadProgress] = useState(0);
  const [fileId, setFileId] = useState("");

  const handleUpload = async (e) => {
    if (e) e.preventDefault();
    if (!selectedFile) return;

    setIsUploading(true);
    setUploadStatus("Encrypting and slicing chunks...");
    setUploadProgress(0);
    setFileId("");

    const storedKey = localStorage.getItem(CONFIG.API_KEY_STORAGE_KEY);
    const formData = new FormData();
    formData.append("file", selectedFile);

    const folderParam = currentFolderId ? `?folder_id=${currentFolderId}` : "";

    try {
      const data = await new Promise((resolve, reject) => {
        const xhr = new XMLHttpRequest();
        xhr.open("POST", `${CONFIG.API_BASE_URL}${API_ROUTES.FILES_UPLOAD}${folderParam}`);
        
        if (storedKey) {
          xhr.setRequestHeader("x-api-key", storedKey);
        }

        xhr.upload.onprogress = (event) => {
          if (event.lengthComputable) {
            const percentComplete = Math.round((event.loaded / event.total) * 100);
            setUploadProgress(percentComplete);
            setUploadStatus(percentComplete < 100 ? "Uploading..." : "Processing and Encrypting...");
          }
        };

        xhr.onload = () => {
          if (xhr.status >= 200 && xhr.status < 300) {
            try {
              resolve(JSON.parse(xhr.responseText));
            } catch (err) {
              reject(new Error("Invalid JSON response"));
            }
          } else {
            let errorMsg = `Upload failed with status ${xhr.status}`;
            try {
              const errorData = JSON.parse(xhr.responseText);
              if (errorData.error) errorMsg = errorData.error;
            } catch (err) {}
            reject(new Error(errorMsg));
          }
        };

        xhr.onerror = () => reject(new Error("Network Error occurred during upload"));
        xhr.send(formData);
      });

      setFileId(data.file_id || data.id);
      setUploadStatus("Upload successful!");
      
      // Refresh the ledger after successful upload
      if (fetchVaultFiles) fetchVaultFiles();
      
    } catch (error) {
      console.error("Upload error:", error);
      setUploadStatus("Error: " + error.message);
    } finally {
      setIsUploading(false);
      setUploadProgress(0);
    }
  };

  const resetUpload = () => {
    setSelectedFile(null);
    setUploadStatus("");
    setUploadProgress(0);
    setFileId("");
  };

  return {
    selectedFile,
    setSelectedFile,
    isUploading,
    uploadStatus,
    uploadProgress,
    fileId,
    setFileId,
    setUploadStatus,
    setUploadProgress,
    handleUpload,
    resetUpload
  };
}
