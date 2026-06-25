"use client";

import React, { useState, useEffect } from "react";
import { Download, Lock, File, AlertTriangle } from "lucide-react";
import { CONFIG } from "@/lib/config";
import { API_ROUTES } from "@/constants/routes";
import { useParams } from "next/navigation";

export default function SharedFilePage() {
  const { token } = useParams();
  
  const [fileInfo, setFileInfo] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [password, setPassword] = useState("");
  const [downloading, setDownloading] = useState(false);
  const [passwordError, setPasswordError] = useState(null);

  useEffect(() => {
    fetchShareInfo();
  }, [token]);

  const fetchShareInfo = async () => {
    try {
      const res = await fetch(`${CONFIG.API_BASE_URL}${API_ROUTES.SHARES_PUBLIC_INFO(token)}`);
      if (!res.ok) {
        if (res.status === 404) throw new Error("Share link not found or revoked.");
        throw new Error("Failed to fetch file information.");
      }
      const data = await res.json();
      setFileInfo(data);
    } catch (err) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  };

  const handleDownload = async (e) => {
    e.preventDefault();
    if (!fileInfo) return;
    
    setDownloading(true);
    setPasswordError(null);
    
    try {
      let downloadUrl = `${CONFIG.API_BASE_URL}${API_ROUTES.SHARES_PUBLIC_DOWNLOAD(token)}`;
      if (fileInfo.requires_password) {
        if (!password) {
          throw new Error("Password is required.");
        }
        downloadUrl += `?pwd=${encodeURIComponent(password)}`;
      }

      const res = await fetch(downloadUrl);
      if (!res.ok) {
        if (res.status === 401) throw new Error("Incorrect password.");
        if (res.status === 410) throw new Error("This share link has expired or reached its download limit.");
        throw new Error("Download failed.");
      }

      // Handle file download
      const blob = await res.blob();
      const url = window.URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = fileInfo.file_name;
      document.body.appendChild(a);
      a.click();
      window.URL.revokeObjectURL(url);
      a.remove();
      
      // Refresh info to see if limit reached
      fetchShareInfo();
      
    } catch (err) {
      setPasswordError(err.message);
    } finally {
      setDownloading(false);
    }
  };

  if (loading) {
    return (
      <main className="min-h-screen bg-[#050505] flex items-center justify-center p-4">
        <div className="animate-pulse flex items-center gap-2 text-zinc-500">
          <div className="w-4 h-4 rounded-full bg-zinc-500 animate-bounce"></div>
          <div className="w-4 h-4 rounded-full bg-zinc-500 animate-bounce delay-100"></div>
          <div className="w-4 h-4 rounded-full bg-zinc-500 animate-bounce delay-200"></div>
        </div>
      </main>
    );
  }

  if (error) {
    return (
      <main className="min-h-screen bg-[#050505] flex flex-col items-center justify-center p-4 text-center">
        <div className="mb-6 p-4 bg-red-500/10 rounded-full">
          <AlertTriangle className="w-12 h-12 text-red-500" />
        </div>
        <h1 className="text-2xl font-bold text-white mb-2">Link Unavailable</h1>
        <p className="text-zinc-400 max-w-md">{error}</p>
      </main>
    );
  }

  if (fileInfo?.is_expired) {
    return (
      <main className="min-h-screen bg-[#050505] flex flex-col items-center justify-center p-4 text-center">
        <div className="mb-6 p-4 bg-amber-500/10 rounded-full">
          <AlertTriangle className="w-12 h-12 text-amber-500" />
        </div>
        <h1 className="text-2xl font-bold text-white mb-2">Link Expired</h1>
        <p className="text-zinc-400 max-w-md">This share link has expired or reached its maximum download limit.</p>
      </main>
    );
  }

  return (
    <main className="min-h-screen bg-[#050505] flex items-center justify-center p-4 font-sans selection:bg-indigo-500/30 text-zinc-100 relative overflow-hidden">
      
      {/* Background aesthetics */}
      <div className="absolute inset-0 z-0">
        <div className="absolute inset-0 bg-[radial-gradient(ellipse_at_center,_var(--tw-gradient-stops))] from-indigo-900/10 via-[#050505]/50 to-[#050505]"></div>
        <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[800px] h-[800px] bg-indigo-500/5 rounded-full blur-3xl mix-blend-screen pointer-events-none"></div>
      </div>

      <div className="relative z-10 w-full max-w-md bg-zinc-900/50 backdrop-blur-xl border border-zinc-800/50 rounded-2xl shadow-2xl p-8">
        
        <div className="text-center mb-8">
          <div className="mx-auto w-16 h-16 bg-indigo-500/10 flex items-center justify-center rounded-2xl mb-4 border border-indigo-500/20">
            <File className="w-8 h-8 text-indigo-400" />
          </div>
          <h1 className="text-xl font-bold text-white mb-1 truncate px-4" title={fileInfo.file_name}>
            {fileInfo.file_name}
          </h1>
          <p className="text-sm text-zinc-400 font-mono">
            {(fileInfo.file_size / 1024 / 1024).toFixed(2)} MB
          </p>
        </div>

        <form onSubmit={handleDownload} className="space-y-6">
          {fileInfo.requires_password && (
            <div className="space-y-2">
              <label className="text-sm text-zinc-400 flex items-center justify-center gap-1">
                <Lock className="w-4 h-4" /> This file is password protected
              </label>
              <input 
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                required
                placeholder="Enter password..."
                className="w-full bg-zinc-950/50 border border-zinc-800 rounded-lg px-4 py-3 text-center text-zinc-100 focus:outline-none focus:border-indigo-500 transition-colors"
              />
            </div>
          )}

          {passwordError && (
            <div className="p-3 bg-red-500/10 border border-red-500/20 rounded-lg text-red-400 text-sm text-center">
              {passwordError}
            </div>
          )}

          <button
            type="submit"
            disabled={downloading}
            className="w-full bg-white text-black hover:bg-zinc-200 font-semibold py-3 px-4 rounded-xl transition-colors flex items-center justify-center gap-2 group disabled:opacity-70"
          >
            {downloading ? (
              <span className="flex items-center gap-2">
                Downloading <span className="animate-pulse">...</span>
              </span>
            ) : (
              <>
                <Download className="w-5 h-5 group-hover:-translate-y-0.5 transition-transform" />
                Download File
              </>
            )}
          </button>
        </form>

        <div className="mt-8 text-center">
          <p className="text-xs text-zinc-500 font-mono tracking-widest uppercase">
            Secured by ForgeCloud
          </p>
        </div>
      </div>
    </main>
  );
}
