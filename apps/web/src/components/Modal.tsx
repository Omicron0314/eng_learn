"use client";
import { type ReactNode, useEffect, useRef } from "react";
export function Modal({
  title,
  onClose,
  children,
}: {
  title: string;
  onClose: () => void;
  children: ReactNode;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const close = useRef(onClose);
  close.current = onClose;
  useEffect(() => {
    const dialog = ref.current;
    dialog?.showModal();
    return () => dialog?.close();
  }, []);
  return (
    <dialog
      ref={ref}
      className="modal"
      aria-label={title}
      onCancel={(e) => {
        e.preventDefault();
        close.current();
      }}
    >
      <div className="section-title">
        <h2>{title}</h2>
        <button type="button" className="text-button" aria-label="关闭弹层" onClick={onClose}>
          ✕
        </button>
      </div>
      {children}
    </dialog>
  );
}
