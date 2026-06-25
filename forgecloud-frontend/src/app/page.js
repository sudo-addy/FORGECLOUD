"use client";

import { useState, useEffect } from "react";
import { motion } from "framer-motion";
import { Lock } from "lucide-react";
import { UploadCard } from "../components/UploadCard";
import { DownloadCard } from "../components/DownloadCard";
import { VaultLedger } from "../components/VaultLedger";
import { PreviewModal } from "../components/PreviewModal";
import { useUpload } from "../hooks/useUpload";
import { useVault } from "../hooks/useVault";

const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL || "http://localhost:3000";

export default function Home() {
  const [apiKey, setApiKey] = useState("");
  const [isAuthorized, setIsAuthorized] = useState(false);
  const [isMounted, setIsMounted] = useState(false);

  // Upload Engine State - Extracted to useUpload


  // Download Engine State
  const [downloadFileId, setDownloadFileId] = useState("");
  const [isDownloading, setIsDownloading] = useState(false);
  const [downloadStatus, setDownloadStatus] = useState("");

  // Vault Ledger State - Extracted to useVault

  // Preview State
  const [previewFile, setPreviewFile] = useState(null);
  const [previewBlobUrl, setPreviewBlobUrl] = useState(null);
  const [previewTextContent, setPreviewTextContent] = useState(null);
  const [isPreviewLoading, setIsPreviewLoading] = useState(false);

  useEffect(() => {
    setIsMounted(true);
    const storedKey = localStorage.getItem("fc_api_key");
    if (storedKey) {
      setApiKey(storedKey);
      setIsAuthorized(true);
    }

    const handleKeyDown = (e) => {
      if (e.key === "Escape") closePreview();
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  const {
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
  } = useVault(isAuthorized);

  // Re-fetch when authorized or folder changes
  useEffect(() => {
    if (isAuthorized) {
      fetchVaultFiles();
    }
  }, [isAuthorized, currentFolderId, fetchVaultFiles]);

  const {
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
  } = useUpload(currentFolderId, fetchVaultFiles);

  const handleLogin = (e) => {
    e.preventDefault();
    if (apiKey.trim()) {
      localStorage.setItem("fc_api_key", apiKey.trim());
      setIsAuthorized(true);
    }
  };

  const handleLogout = () => {
    localStorage.removeItem("fc_api_key");
    setApiKey("");
    setIsAuthorized(false);
    resetUpload();
    resetVault();
    setDownloadFileId("");
    setDownloadStatus("");
    closePreview();
  };

  const closePreview = () => {
    setPreviewFile(null);
    setPreviewTextContent(null);
    setIsPreviewLoading(false);
    if (previewBlobUrl) {
      URL.revokeObjectURL(previewBlobUrl);
      setPreviewBlobUrl(null);
    }
  };

  const handlePreview = async (file) => {
    const mime = file.mime_type || "";
    const isImage = mime.startsWith("image/");
    const isVideo = mime.startsWith("video/");
    const isAudio = mime.startsWith("audio/");
    const isPdf = mime === "application/pdf";
    const isText = mime.startsWith("text/") || mime === "application/json";

    if (!isImage && !isVideo && !isAudio && !isPdf && !isText) {
      alert("Preview not available for this file type. Download required.");
      return;
    }

    setPreviewFile(file);
    setIsPreviewLoading(true);

    const storedKey = localStorage.getItem("fc_api_key");
    try {
      const response = await fetch(`${API_BASE_URL}/v1/files/download/${file.id}`, {
        method: "GET",
        headers: { "x-api-key": storedKey },
      });

      if (!response.ok) throw new Error("Failed to load preview");

      const blob = await response.blob();
      
      if (isText) {
        const text = await blob.text();
        setPreviewTextContent(text);
      } else {
        const url = URL.createObjectURL(blob);
        setPreviewBlobUrl(url);
      }
    } catch (error) {
      console.error("Preview error:", error);
      alert("Failed to load preview: " + error.message);
      closePreview();
    } finally {
      setIsPreviewLoading(false);
    }
  };

  const handleDownload = async (e, specificId = null) => {
    if (e) e.preventDefault();
    
    const targetId = specificId || downloadFileId.trim();
    if (!targetId) return;

    setIsDownloading(true);
    setDownloadStatus("Connecting to Rust ingestion core...");

    const storedKey = localStorage.getItem("fc_api_key");

    try {
      const response = await fetch(`${API_BASE_URL}/v1/files/download/${targetId}`, {
        method: "GET",
        headers: {
          "x-api-key": storedKey,
        },
      });

      if (response.status === 401) {
        throw new Error("Unauthorized key");
      }
      if (response.status === 404) {
        throw new Error("File not found in Neon DB");
      }
      if (!response.ok) {
        throw new Error(`Download failed with status ${response.status}`);
      }

      // Read Content-Disposition header to parse original filename
      let filename = "downloaded_file.bin";
      const contentDisposition = response.headers.get("content-disposition");
      if (contentDisposition && contentDisposition.indexOf("filename=") !== -1) {
        const filenameMatch = contentDisposition.match(/filename="?([^"]+)"?/);
        if (filenameMatch && filenameMatch.length === 2) {
          filename = filenameMatch[1];
        }
      }

      const blob = await response.blob();
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = filename;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);

      setDownloadStatus("Download complete!");
    } catch (error) {
      console.error("Download error:", error);
      setDownloadStatus("Error: " + error.message);
    } finally {
      setIsDownloading(false);
    }
  };

  if (!isMounted) return null;

  if (!isAuthorized) {
    return (
      <div className="relative min-h-screen bg-black text-white flex flex-col items-center justify-center overflow-hidden">
        {/* Deep Space Canvas Lighting */}
        <div className="absolute inset-0 bg-[radial-gradient(ellipse_at_top,_var(--tw-gradient-stops))] from-indigo-900/20 via-black to-black pointer-events-none" />
        
        {/* Neon Eclipse Ring */}
        <div className="absolute w-[40rem] h-[40rem] rounded-full border-[2px] border-indigo-500 opacity-20 shadow-[0_0_120px_30px_rgba(99,102,241,0.4),_inset_0_0_80px_20px_rgba(99,102,241,0.4)] pointer-events-none" />

        <motion.div 
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.8, ease: "easeOut" }}
          className="relative z-10"
        >
          <form onSubmit={handleLogin} className="flex flex-col items-center gap-8">
            <label className="text-4xl sm:text-5xl tracking-[0.5em] font-light text-white text-center ml-4">
              ENTER VAULT KEY
            </label>
            <input
              type="password"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              className="bg-black/50 backdrop-blur-md border-b border-zinc-800 text-white px-4 py-3 focus:outline-none focus:border-indigo-500 w-80 text-center tracking-widest transition-colors shadow-[0_4px_30px_rgba(0,0,0,0.1)]"
              autoFocus
            />
          </form>
        </motion.div>
      </div>
    );
  }

  return (
    <div className="relative min-h-screen bg-black text-white flex flex-col items-center p-4 overflow-x-hidden overflow-y-auto pt-16 pb-24">
      {/* Deep Space Canvas Lighting */}
      <div className="fixed inset-0 bg-[radial-gradient(ellipse_at_top,_var(--tw-gradient-stops))] from-indigo-900/20 via-black to-black pointer-events-none" />

      <motion.div 
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ duration: 1 }}
        className="relative z-10 w-full max-w-5xl flex flex-col items-center gap-12"
      >
        <div className="flex flex-col md:flex-row gap-8 w-full justify-center">
          
          {/* Upload Card */}
          <UploadCard 
            selectedFile={selectedFile}
            setSelectedFile={setSelectedFile}
            isUploading={isUploading}
            uploadProgress={uploadProgress}
            uploadStatus={uploadStatus}
            fileId={fileId}
            handleUpload={handleUpload}
          />

          {/* Download Card */}
          <DownloadCard 
            downloadFileId={downloadFileId}
            setDownloadFileId={setDownloadFileId}
            isDownloading={isDownloading}
            downloadStatus={downloadStatus}
            handleDownload={handleDownload}
          />
        </div>

        {/* Vault Ledger */}
        <VaultLedger
          isAuthorized={isAuthorized}
          vaultFiles={vaultFiles}
          folders={folders}
          isFetchingFiles={isFetchingFiles}
          newFolderName={newFolderName}
          setNewFolderName={setNewFolderName}
          isCreatingFolder={isCreatingFolder}
          handleCreateFolder={handleCreateFolder}
          breadcrumbs={breadcrumbs}
          currentFolderId={currentFolderId}
          setCurrentFolderId={setCurrentFolderId}
          setBreadcrumbs={setBreadcrumbs}
          handlePreview={handlePreview}
          handleMoveFile={handleMoveFile}
          handleDownload={handleDownload}
          handleDelete={handleDelete}
          isDownloading={isDownloading}
        />

        <motion.button
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ delay: 0.6 }}
          onClick={handleLogout}
          className="flex items-center gap-2 px-6 py-2 text-xs tracking-[0.2em] text-zinc-400 hover:text-red-400 transition-colors group relative z-10 mt-8"
        >
          <Lock size={14} className="group-hover:scale-110 transition-transform" />
          LOCK VAULT
        </motion.button>
      </motion.div>

      {/* Preview Modal */}
      <PreviewModal 
        previewFile={previewFile}
        previewBlobUrl={previewBlobUrl}
        previewTextContent={previewTextContent}
        isPreviewLoading={isPreviewLoading}
        closePreview={closePreview}
      />
    </div>
  );
}
