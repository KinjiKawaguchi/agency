import { createContext, useCallback, useContext, useMemo, useState, type ReactNode } from 'react'
import './toasts.css'

export type ToastTone = 'info' | 'error'

interface Toast {
  id: number
  message: string
  tone: ToastTone
}

const LIFETIME_MS: Record<ToastTone, number> = { info: 4500, error: 8000 }

const ToastContext = createContext<((message: string, tone?: ToastTone) => void) | null>(null)

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<readonly Toast[]>([])

  const show = useCallback((message: string, tone: ToastTone = 'info') => {
    const id = Date.now() + Math.random()
    setToasts((current) => [...current, { id, message, tone }])
    window.setTimeout(() => setToasts((current) => current.filter((toast) => toast.id !== id)), LIFETIME_MS[tone])
  }, [])

  const value = useMemo(() => show, [show])
  return (
    <ToastContext.Provider value={value}>
      {children}
      <div className="toasts" role="status" aria-live="polite">
        {toasts.map((toast) => (
          <div key={toast.id} className={`toast ${toast.tone}`}>
            {toast.message}
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  )
}

export function useToast(): (message: string, tone?: ToastTone) => void {
  const show = useContext(ToastContext)
  if (show === null) throw new Error('useToast must be used inside ToastProvider')
  return show
}
