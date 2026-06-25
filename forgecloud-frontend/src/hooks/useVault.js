import { useState, useCallback } from "react";
import { CONFIG } from "../lib/config";
import { API_ROUTES } from "../constants/routes";
import { getAuthHeaders } from "../lib/api";

export function useVault(isAuthorized) {
  const [vaultFiles, setVaultFiles] = useState([]);
  const [folders, setFolders] = useState([]);
  const [currentFolderId, setCurrentFolderId] = useState(null);
  const [breadcrumbs, setBreadcrumbs] = useState([{ id: null, name: "Root" }]);
  const [isFetchingFiles, setIsFetchingFiles] = useState(false);
  
  const [newFolderName, setNewFolderName] = useState("");
  const [isCreatingFolder, setIsCreatingFolder] = useState(false);

  const fetchVaultFiles = useCallback(async () => {
    setIsFetchingFiles(true);
    const folderParam = currentFolderId ? `?folder_id=${currentFolderId}` : "";
    const folderFetchParam = currentFolderId ? `?parent_id=${currentFolderId}` : "";
    
    try {
      const headers = getAuthHeaders();
      const [filesRes, foldersRes] = await Promise.all([
        fetch(`${CONFIG.API_BASE_URL}${API_ROUTES.FILES}${folderParam}`, { headers }),
        fetch(`${CONFIG.API_BASE_URL}${API_ROUTES.FOLDERS}${folderFetchParam}`, { headers })
      ]);
      
      if (filesRes.ok && foldersRes.ok) {
        const filesData = await filesRes.json();
        const foldersData = await foldersRes.json();
        setVaultFiles(filesData);
        setFolders(foldersData);
      } else {
        console.error("Failed to fetch from Vault");
      }
    } catch (error) {
      console.error("Error fetching Vault data:", error);
    } finally {
      setIsFetchingFiles(false);
    }
  }, [currentFolderId]);

  const handleCreateFolder = async (e) => {
    e.preventDefault();
    if (!newFolderName.trim()) return;

    setIsCreatingFolder(true);

    try {
      const response = await fetch(`${CONFIG.API_BASE_URL}${API_ROUTES.FOLDERS}`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          ...getAuthHeaders()
        },
        body: JSON.stringify({
          name: newFolderName.trim(),
          parent_id: currentFolderId,
        }),
      });

      if (response.ok) {
        setNewFolderName("");
        fetchVaultFiles();
      }
    } catch (error) {
      console.error("Error creating folder:", error);
    } finally {
      setIsCreatingFolder(false);
    }
  };

  const handleMoveFile = async (fileId) => {
    const destId = prompt("Enter Destination Folder UUID (leave blank to move to Root):");
    if (destId === null) return;

    try {
      const response = await fetch(`${CONFIG.API_BASE_URL}${API_ROUTES.FILES_UPDATE(fileId)}`, {
        method: "PATCH",
        headers: {
          "Content-Type": "application/json",
          ...getAuthHeaders()
        },
        body: JSON.stringify({ folder_id: destId.trim() || null }),
      });

      if (response.ok) {
        fetchVaultFiles();
      } else {
        alert("Failed to move file. Ensure destination UUID is correct.");
      }
    } catch (error) {
      console.error("Error moving file:", error);
    }
  };

  const handleDelete = async (fileId) => {
    if (!confirm("Are you sure you want to permanently delete this file?")) return;

    try {
      const response = await fetch(`${CONFIG.API_BASE_URL}${API_ROUTES.FILES_DELETE(fileId)}`, {
        method: "DELETE",
        headers: getAuthHeaders(),
      });

      if (!response.ok) {
        throw new Error("Failed to delete file");
      }
      
      fetchVaultFiles();
    } catch (error) {
      console.error("Delete error:", error);
      alert("Error: " + error.message);
    }
  };

  const resetVault = () => {
    setVaultFiles([]);
    setFolders([]);
    setCurrentFolderId(null);
    setBreadcrumbs([{ id: null, name: "Root" }]);
    setNewFolderName("");
  };

  return {
    vaultFiles,
    folders,
    currentFolderId,
    setCurrentFolderId,
    breadcrumbs,
    setBreadcrumbs,
    isFetchingFiles,
    newFolderName,
    setNewFolderName,
    isCreatingFolder,
    fetchVaultFiles,
    handleCreateFolder,
    handleMoveFile,
    handleDelete,
    resetVault
  };
}
