import { useCallback, useEffect, useState } from 'react'
import { loadHomeNodes, saveHomeNodes, type HomeNodes } from './homeNodes'

/** Said when the list changes, so every part of the page that shows it reads it again. */
const CHANGED = 'agency:home-nodes'

/** The list of other home nodes of this origin, kept in its `localStorage`. */
export function useHomeNodes(): { nodes: HomeNodes; change: (next: (current: HomeNodes) => HomeNodes) => void } {
  const [nodes, setNodes] = useState<HomeNodes>(() => loadHomeNodes())
  useEffect(() => {
    const reread = (): void => setNodes(loadHomeNodes())
    window.addEventListener(CHANGED, reread)
    return () => window.removeEventListener(CHANGED, reread)
  }, [])
  const change = useCallback((next: (current: HomeNodes) => HomeNodes) => {
    const updated = next(loadHomeNodes())
    saveHomeNodes(updated)
    setNodes(updated)
    window.dispatchEvent(new Event(CHANGED))
  }, [])
  return { nodes, change }
}
