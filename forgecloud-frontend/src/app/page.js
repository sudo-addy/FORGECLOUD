"use client";

import { useState, useEffect } from "react";
import { motion } from "framer-motion";
import { UploadCloud, DownloadCloud, Lock, File as FileIcon, CheckCircle, AlertCircle } from "lucide-react";

export default function Home() {
  const [apiKey, setApiKey] = useState("");
  const [isAuthorized, setIsAuthorized] = useState(false);
  const [isMounted, setIsMounted] = useState(false);

  // Upload Engine State
  const [selectedFile, setSelectedFile] = useState(null);
  const [isUploading, setIsUploading] = useState(false);
  const [uploadStatus, setUploadStatus] = useState("");
  const [fileId, setFileId] = useState("");

  // Download Engine State
  const [downloadFileId, setDownloadFileId] = useState("");
  const [isDownloading, setIsDownloading] = useState(false);
  const [downloadStatus, setDownloadStatus] = useState("");

  // Vault Ledger State
  const [vaultFiles, setVaultFiles] = useState([]);
  const [isFetchingFiles, setIsFetchingFiles] = useState(false);

  useEffect(() => {
    setIsMounted(true);
    const storedKey = localStorage.getItem("fc_api_key");
    if (storedKey) {
      setApiKey(storedKey);
      setIsAuthorized(true);
    }
  }, []);

  // Fetch Vault Files when authorized
  useEffect(() => {
    if (isAuthorized) {
      fetchVaultFiles();
    }
  }, [isAuthorized]);

  const fetchVaultFiles = async () => {
    setIsFetchingFiles(true);
    const storedKey = localStorage.getItem("fc_api_key");
    try {
      const response = await fetch("http://localhost:3000/v1/files", {
        method: "GET",
        headers: {
          "x-api-key": storedKey,
        },
      });
      if (response.ok) {
        const data = await response.json();
        setVaultFiles(data);
      } else {
        console.error("Failed to fetch files from Vault");
      }
    } catch (error) {
      console.error("Error fetching Vault files:", error);
    } finally {
      setIsFetchingFiles(false);
    }
  };

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
    setSelectedFile(null);
    setUploadStatus("");
    setFileId("");
    setDownloadFileId("");
    setDownloadStatus("");
    setVaultFiles([]);
  };

  const handleUpload = async (e) => {
    e.preventDefault();
    if (!selectedFile) return;

    setIsUploading(true);
    setUploadStatus("Encrypting and slicing chunks...");
    setFileId("");

    const storedKey = localStorage.getItem("fc_api_key");
    const formData = new FormData();
    formData.append("file", selectedFile);

    try {
      const response = await fetch("http://localhost:3000/v1/files/upload", {
        method: "POST",
        headers: {
          "x-api-key": storedKey,
        },
        body: formData,
      });

      if (!response.ok) {
        const errorData = await response.json().catch(() => ({}));
        throw new Error(errorData.error || `Upload failed with status ${response.status}`);
      }

      const data = await response.json();
      setFileId(data.id);
      setUploadStatus("Upload successful!");
      
      // Refresh the ledger after successful upload
      fetchVaultFiles();
      
    } catch (error) {
      console.error("Upload error:", error);
      setUploadStatus("Error: " + error.message);
    } finally {
      setIsUploading(false);
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
      const response = await fetch(`http://localhost:3000/v1/files/download/${targetId}`, {
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
          <motion.div 
            initial={{ opacity: 0, y: 40 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.8, delay: 0.1, ease: "easeOut" }}
            className="bg-gradient-to-b from-white/[0.08] to-transparent border-t border-white/20 border-x-0 border-b-0 backdrop-blur-xl shadow-2xl rounded-2xl p-8 flex flex-col items-center w-full md:w-96 gap-6 relative group"
          >
            <div className="absolute inset-0 bg-gradient-to-b from-white/[0.02] to-transparent rounded-2xl pointer-events-none" />
            <div className="flex items-center gap-3 relative z-10">
              <UploadCloud className="text-indigo-400" size={28} />
              <h2 className="text-xl font-light tracking-[0.2em] text-white">UPLOAD</h2>
            </div>
            
            <form onSubmit={handleUpload} className="flex flex-col items-center gap-6 w-full relative z-10">
              <label className="w-full relative cursor-pointer group/upload">
                <input 
                  type="file" 
                  onChange={(e) => setSelectedFile(e.target.files[0])}
                  className="hidden"
                />
                <div className="w-full border border-dashed border-white/20 rounded-xl p-6 flex flex-col items-center justify-center gap-2 bg-black/20 group-hover/upload:border-indigo-400/50 group-hover/upload:bg-indigo-900/10 transition-all">
                  <FileIcon className={selectedFile ? "text-indigo-400" : "text-zinc-600"} size={24} />
                  <span className="text-xs tracking-wider text-zinc-400 text-center truncate w-full px-2">
                    {selectedFile ? selectedFile.name : "SELECT FILE"}
                  </span>
                </div>
              </label>
              
              <button 
                type="submit" 
                disabled={isUploading || !selectedFile}
                className="w-full py-3 bg-white/10 border border-white/10 text-white text-sm tracking-[0.1em] rounded hover:bg-white/20 hover:shadow-[0_0_20px_rgba(255,255,255,0.2)] disabled:opacity-30 disabled:cursor-not-allowed transition-all"
              >
                {isUploading ? "UPLOADING..." : "EXECUTE SECURE UPLOAD"}
              </button>
            </form>

            {(uploadStatus || fileId) && (
              <div className="w-full mt-2 relative z-10">
                {uploadStatus && (
                  <p className="text-xs text-indigo-300 tracking-wider text-center mb-4 flex items-center justify-center gap-2">
                    {uploadStatus.includes("Error") ? <AlertCircle size={14} className="text-red-400" /> : <CheckCircle size={14} />}
                    {uploadStatus}
                  </p>
                )}
                
                {fileId && (
                  <div className="w-full p-4 bg-black/40 border border-white/5 rounded-lg text-center overflow-hidden backdrop-blur-md">
                    <p className="text-[10px] text-zinc-500 mb-2 tracking-[0.3em]">SECURE FILE ID</p>
                    <p className="text-xs font-mono text-white break-all select-all">{fileId}</p>
                  </div>
                )}
              </div>
            )}
          </motion.div>

          {/* Download Card */}
          <motion.div 
            initial={{ opacity: 0, y: 40 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.8, delay: 0.2, ease: "easeOut" }}
            className="bg-gradient-to-b from-white/[0.08] to-transparent border-t border-white/20 border-x-0 border-b-0 backdrop-blur-xl shadow-2xl rounded-2xl p-8 flex flex-col items-center w-full md:w-96 gap-6 relative"
          >
            <div className="absolute inset-0 bg-gradient-to-b from-white/[0.02] to-transparent rounded-2xl pointer-events-none" />
            <div className="flex items-center gap-3 relative z-10">
              <DownloadCloud className="text-indigo-400" size={28} />
              <h2 className="text-xl font-light tracking-[0.2em] text-white">DOWNLOAD</h2>
            </div>
            
            <form onSubmit={handleDownload} className="flex flex-col items-center gap-6 w-full relative z-10">
              <div className="w-full relative">
                <input 
                  type="text" 
                  placeholder="PASTE UUID..."
                  value={downloadFileId}
                  onChange={(e) => setDownloadFileId(e.target.value)}
                  className="w-full bg-black/40 border border-white/10 text-white rounded-xl px-4 py-4 focus:outline-none focus:border-indigo-400/50 text-center text-xs tracking-widest placeholder:text-zinc-600 transition-colors"
                />
              </div>
              
              <button 
                type="submit" 
                disabled={isDownloading || !downloadFileId.trim()}
                className="w-full py-3 bg-white/10 border border-white/10 text-white text-sm tracking-[0.1em] rounded hover:bg-white/20 hover:shadow-[0_0_20px_rgba(255,255,255,0.2)] disabled:opacity-30 disabled:cursor-not-allowed transition-all"
              >
                {isDownloading ? "DECRYPTING..." : "INITIATE TRANSFER"}
              </button>
            </form>

            {downloadStatus && (
              <p className="text-xs text-indigo-300 tracking-wider text-center mt-2 flex items-center justify-center gap-2 relative z-10">
                {downloadStatus.includes("Error") ? <AlertCircle size={14} className="text-red-400" /> : <CheckCircle size={14} />}
                {downloadStatus}
              </p>
            )}
          </motion.div>
        </div>

        {/* Vault Ledger */}
        <motion.div 
          initial={{ opacity: 0, y: 40 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.8, delay: 0.3, ease: "easeOut" }}
          className="w-full max-w-4xl mt-4 flex flex-col gap-4 relative z-10"
        >
          <h2 className="text-xs font-light tracking-[0.4em] text-indigo-400 mb-2 pl-2 shadow-indigo-500/20 drop-shadow-md">ENCRYPTED LEDGER</h2>
          
          {isFetchingFiles && vaultFiles.length === 0 ? (
            <p className="text-zinc-500 text-xs tracking-widest pl-2">SYNCING WITH VAULT...</p>
          ) : vaultFiles.length === 0 ? (
            <p className="text-zinc-500 text-xs tracking-widest pl-2">NO FILES DETECTED IN VAULT.</p>
          ) : (
            <div className="flex flex-col gap-3">
              {vaultFiles.map((file, i) => (
                <motion.div 
                  key={file.id}
                  initial={{ opacity: 0, x: -20 }}
                  animate={{ opacity: 1, x: 0 }}
                  transition={{ duration: 0.5, delay: 0.4 + (i * 0.05), ease: "easeOut" }}
                  className="bg-gradient-to-r from-white/[0.05] to-transparent border-t border-white/10 backdrop-blur-md rounded-lg p-5 flex flex-col sm:flex-row justify-between items-start sm:items-center gap-4 group hover:bg-white/[0.08] transition-colors"
                >
                  <div className="flex flex-col gap-1">
                    <span className="font-semibold text-white tracking-wide">{file.name}</span>
                    <span className="text-xs text-zinc-500 tracking-wider">
                      {new Date(file.created_at).toLocaleString()} &nbsp;•&nbsp; {(file.total_size / 1024).toFixed(2)} KB
                    </span>
                  </div>
                  
                  <div className="flex items-center gap-6 w-full sm:w-auto justify-between sm:justify-end">
                    <span className="text-xs font-mono text-zinc-600 tracking-widest truncate max-w-[150px] sm:max-w-none">{file.id}</span>
                    <button 
                      onClick={() => handleDownload(null, file.id)}
                      disabled={isDownloading}
                      className="p-2 bg-indigo-500/10 text-indigo-400 hover:bg-indigo-500/20 hover:text-indigo-300 rounded transition-colors disabled:opacity-50"
                      title="Download File"
                    >
                      <DownloadCloud size={18} />
                    </button>
                  </div>
                </motion.div>
              ))}
            </div>
          )}
        </motion.div>

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
    </div>
  );
}
