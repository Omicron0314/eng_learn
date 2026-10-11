"use client";
import { useEffect, useRef, useState } from "react";
import type { AudioResult } from "../server/services";
import { Notice } from "./ui";
export function AudioPlayer({
  source,
  onEnded,
  onStarted,
}: {
  source: AudioResult;
  onEnded: () => void;
  onStarted: (rate: number) => void;
}) {
  const audio = useRef<HTMLAudioElement>(null);
  const utterance = useRef<SpeechSynthesisUtterance | null>(null);
  const [speed, setSpeed] = useState(1);
  const [error, setError] = useState("");
  useEffect(() => {
    const value = Number(localStorage.getItem("audio-rate"));
    if ([0.75, 0.9, 1, 1.1, 1.25].includes(value)) setSpeed(value);
    return () => {
      if ("speechSynthesis" in window) window.speechSynthesis.cancel();
    };
  }, []);
  useEffect(() => {
    if (audio.current) audio.current.playbackRate = speed;
    if (utterance.current) utterance.current.rate = speed;
  }, [speed]);
  async function play() {
    setError("");
    try {
      if (source.mode === "server") await audio.current?.play();
      else {
        if (!("speechSynthesis" in window))
          throw new Error("当前浏览器不支持朗读，请尝试其他浏览器。");
        if (window.speechSynthesis.paused) window.speechSynthesis.resume();
        else {
          window.speechSynthesis.cancel();
          const item = new SpeechSynthesisUtterance(source.text);
          item.lang = "en-US";
          item.rate = speed;
          item.onend = onEnded;
          item.onerror = (event) => {
            if (event.error !== "canceled" && event.error !== "interrupted")
              setError("浏览器朗读失败，请重试。");
          };
          utterance.current = item;
          window.speechSynthesis.speak(item);
        }
        onStarted(speed);
      }
    } catch (err) {
      setError((err as Error).message);
    }
  }
  return (
    <div className="audio-player">
      <div className="section-title">
        <span className="pill">
          {source.mode === "server" ? "整篇音频" : "浏览器朗读 · 不支持拖动进度"}
        </span>
        <label className="rate-label">
          播放速度
          <select
            value={speed}
            onChange={(e) => {
              const value = Number(e.target.value);
              setSpeed(value);
              localStorage.setItem("audio-rate", String(value));
            }}
          >
            {[0.75, 0.9, 1, 1.1, 1.25].map((rate) => (
              <option key={rate} value={rate}>
                {rate}×
              </option>
            ))}
          </select>
        </label>
      </div>
      {source.mode === "server" && (
        <audio
          ref={audio}
          controls
          src={source.url}
          onPlay={() => onStarted(speed)}
          onEnded={onEnded}
        >
          <track kind="captions" />
        </audio>
      )}
      <div className="actions">
        <button type="button" onClick={play}>
          播放
        </button>
        <button
          type="button"
          className="secondary"
          onClick={() => {
            if (source.mode === "server") audio.current?.pause();
            else window.speechSynthesis?.pause();
          }}
        >
          暂停
        </button>
        {source.mode === "server" ? (
          <button
            type="button"
            className="text-button"
            onClick={() => {
              if (audio.current)
                audio.current.currentTime = Math.max(0, audio.current.currentTime - 5);
            }}
          >
            后退 5 秒
          </button>
        ) : (
          <button
            type="button"
            className="text-button"
            onClick={() => window.speechSynthesis?.cancel()}
          >
            停止
          </button>
        )}
      </div>
      <Notice error={error} />
    </div>
  );
}
