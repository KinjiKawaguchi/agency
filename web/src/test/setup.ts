import '@testing-library/jest-dom/vitest'
import { cleanup } from '@testing-library/react'
import { afterEach, beforeEach } from 'vitest'
import { MOCK_TOKEN } from '../mock/node'

// The served index.html carries the page's token; the tests' page carries the mock node's.
beforeEach(() => {
  const meta = document.createElement('meta')
  meta.name = 'agency-token'
  meta.content = MOCK_TOKEN
  document.head.append(meta)
})

afterEach(() => {
  document.head.querySelector('meta[name="agency-token"]')?.remove()
  cleanup()
  window.localStorage.clear()
  window.sessionStorage.clear()
  window.history.replaceState(null, '', '/')
})

// jsdom does not implement scrolling; the router calls it on navigation.
window.scrollTo = () => undefined
