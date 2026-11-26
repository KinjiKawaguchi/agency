import { useQuery } from '@tanstack/react-query'
import { useCallback, useEffect, useState } from 'react'
import { useApi } from '../api/context'
import { meQuery } from '../api/queries'
import { listedNodes, loadHomeNodes, saveHomeNodes, type HomeNodes, type ListedNode } from './homeNodes'

/** Said when the list changes, so every part of the page that shows it reads it again. */
const CHANGED = 'agency:home-nodes'

/**
 * The other home nodes of this origin: the owner's own, kept in its `localStorage` (`nodes`, which `change` edits), and
 * `listed`, what the page shows, which also holds the ones the node's configuration names.
 */
export function useHomeNodes(): { nodes: HomeNodes; listed: readonly ListedNode[]; change: (next: (current: HomeNodes) => HomeNodes) => void } {
  const api = useApi()
  const fromNode = useQuery(meQuery(api)).data?.homeNodes
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
  return { nodes, listed: listedNodes(nodes, fromNode, window.location.origin), change }
}
