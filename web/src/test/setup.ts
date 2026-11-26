import '@testing-library/jest-dom/vitest'
import { cleanup, configure } from '@testing-library/react'
import { afterEach, beforeEach } from 'vitest'
import { MOCK_TOKEN } from '../mock/node'

// Making the device key (WebCrypto) can take longer than a second on a busy machine, as it does the first
// time after a build. Waiting longer costs nothing when the answer comes sooner.
configure({ asyncUtilTimeout: 5000 })

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
