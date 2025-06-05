import React, { useRef, useState } from 'react';
import { useAnimationPlayerContext } from '../../hooks/useAnimationPlayerContext.js';

const FileUpload = () => {
  const { loadAnimationFromData, isLoading } = useAnimationPlayerContext();
  const [isDragOver, setIsDragOver] = useState(false);
  const [uploadStatus, setUploadStatus] = useState('');
  const fileInputRef = useRef(null);

  const handleFileSelect = async (file) => {
    if (!file) return;

    // Validate file type
    if (!file.name.toLowerCase().endsWith('.json')) {
      setUploadStatus('Please select a JSON file');
      return;
    }

    setUploadStatus('Loading animation...');

    try {
      const text = await file.text();
      const animationData = JSON.parse(text);

      // Extract animation name from file name (remove .json extension)
      const animationName = file.name.replace(/\.json$/i, '');

      // Load the animation data into a new player
      const playerId = await loadAnimationFromData(animationData);
      setUploadStatus(`Successfully loaded: ${file.name} as "${playerId}"`);

      // Clear status after 3 seconds
      setTimeout(() => setUploadStatus(''), 3000);
    } catch (error) {
      console.error('Failed to load animation file:', error);
      setUploadStatus(`Error loading file: ${error.message}`);

      // Clear error after 5 seconds
      setTimeout(() => setUploadStatus(''), 5000);
    }
  };

  const handleFileInputChange = (event) => {
    const file = event.target.files[0];
    handleFileSelect(file);
    // Reset input so same file can be selected again
    event.target.value = '';
  };

  const handleDrop = (event) => {
    event.preventDefault();
    setIsDragOver(false);

    const files = event.dataTransfer.files;
    if (files.length > 0) {
      handleFileSelect(files[0]);
    }
  };

  const handleDragOver = (event) => {
    event.preventDefault();
    setIsDragOver(true);
  };

  const handleDragLeave = (event) => {
    event.preventDefault();
    setIsDragOver(false);
  };

  const openFileDialog = () => {
    fileInputRef.current?.click();
  };

  const loadDemoAnimation = async () => {
    setUploadStatus('Loading demo animation...');
    try {
      const response = await fetch('/test_animation.json');
      if (!response.ok) {
        throw new Error(`Failed to fetch demo file: ${response.statusText}`);
      }
      const animationData = await response.json();
      await loadAnimationFromData(animationData);
      setUploadStatus('Demo animation loaded successfully!');
      setTimeout(() => setUploadStatus(''), 3000);
    } catch (error) {
      console.error('Failed to load demo animation:', error);
      setUploadStatus(`Error loading demo: ${error.message}`);
      setTimeout(() => setUploadStatus(''), 5000);
    }
  };

  return (
    <div className="control-panel">
      <h3>📁 Load Animation Data</h3>

      {/* Hidden file input */}
      <input
        ref={fileInputRef}
        type="file"
        accept=".json"
        onChange={handleFileInputChange}
        style={{ display: 'none' }}
      />

      {/* Drop zone */}
      <div
        className={`file-drop-zone ${isDragOver ? 'drag-over' : ''}`}
        onDrop={handleDrop}
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onClick={openFileDialog}
      >
        <div className="drop-zone-content">
          <div className="drop-zone-icon">📄</div>
          <div className="drop-zone-text">
            <strong>Click to select</strong> or <strong>drag & drop</strong>
            <br />
            JSON animation files
          </div>
        </div>
      </div>

      {/* Status message */}
      {uploadStatus && (
        <div className={`upload-status ${uploadStatus.includes('Error') ? 'error' : 'success'}`}>
          {uploadStatus}
        </div>
      )}

      {/* File format info */}
      <div className="file-format-info">
        <small>
          <strong>Expected JSON format:</strong> Animation data with tracks, keypoints, and timing information.
          <br />
          See the demo animation for an example structure.
        </small>
      </div>

      {/* Quick load button for demo file */}
      <div className="quick-actions">
        <h4>Quick Actions</h4>
        <button
          className="btn-info"
          onClick={loadDemoAnimation}
          disabled={isLoading}
        >
          Load Demo Animation
        </button>
      </div>
    </div>
  );
};

export default FileUpload;
