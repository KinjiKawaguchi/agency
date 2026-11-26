import { askToKeep, forgetKeyLost, keepingNow, recordKeyLoad, wasKeyLost } from './keyKeeping'

const storage = (value: unknown): void => void Object.defineProperty(navigator, 'storage', { value, configurable: true })

describe('whether the browser will keep the key', () => {
  afterEach(() => Reflect.deleteProperty(navigator, 'storage'))

  it('reads what the browser has promised, without asking it for anything', async () => {
    const persist = vi.fn()
    storage({ persisted: () => Promise.resolve(true), persist })
    expect(await keepingNow()).toBe('kept')
    storage({ persisted: () => Promise.resolve(false), persist })
    expect(await keepingNow()).toBe('at-risk')
    expect(persist).not.toHaveBeenCalled()
  })

  it('says it does not know when the browser cannot say, or fails', async () => {
    storage(undefined)
    expect(await keepingNow()).toBe('unknown')
    expect(await askToKeep()).toBe('unknown')
    storage({ persisted: () => Promise.reject(new Error('no')), persist: () => Promise.reject(new Error('no')) })
    expect(await keepingNow()).toBe('unknown')
    expect(await askToKeep()).toBe('unknown')
  })

  it('asks the browser to keep it, and tells whether it agreed', async () => {
    storage({ persist: () => Promise.resolve(true) })
    expect(await askToKeep()).toBe('kept')
    storage({ persist: () => Promise.resolve(false) })
    expect(await askToKeep()).toBe('at-risk')
  })
})

describe('whether an earlier key was lost', () => {
  it('is not lost on a first visit, nor when the key is found again', () => {
    recordKeyLoad(true)
    expect(wasKeyLost()).toBe(false)
    recordKeyLoad(false)
    recordKeyLoad(false)
    expect(wasKeyLost()).toBe(false)
  })

  it('is lost when a key had to be made after one was noted', () => {
    recordKeyLoad(false)
    recordKeyLoad(true)
    expect(wasKeyLost()).toBe(true)
  })

  it('stays lost until it is forgotten, whatever loads in between', () => {
    recordKeyLoad(false)
    recordKeyLoad(true)
    recordKeyLoad(false)
    expect(wasKeyLost()).toBe(true)
    forgetKeyLost()
    expect(wasKeyLost()).toBe(false)
  })

  it('notes a key that was made before this note existed, so that its loss is seen', () => {
    recordKeyLoad(false)
    expect(window.localStorage.getItem('agency.deviceKeyMade')).not.toBeNull()
  })
})
