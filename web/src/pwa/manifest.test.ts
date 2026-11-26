import { existsSync, readFileSync, statSync } from 'node:fs'
import { join } from 'node:path'

const publicDir = join(process.cwd(), 'public')
const manifest = JSON.parse(readFileSync(join(publicDir, 'manifest.webmanifest'), 'utf8')) as {
  id: string
  name: string
  short_name: string
  start_url: string
  scope: string
  display: string
  background_color: string
  theme_color: string
  icons: Array<{ src: string; sizes: string; type: string; purpose?: string }>
  shortcuts: Array<{ name: string; url: string }>
}

/** The width and height a PNG says it has. */
const pngSize = (path: string): string => {
  const bytes = readFileSync(path)
  return `${bytes.readUInt32BE(16)}x${bytes.readUInt32BE(20)}`
}

describe('the app manifest', () => {
  it('opens the desk, standalone, from the whole origin', () => {
    expect([manifest.start_url, manifest.scope, manifest.display, manifest.id]).toEqual(['/desk', '/', 'standalone', '/desk'])
    expect(manifest.name).toBe('Agency')
    expect(manifest.short_name.length).toBeLessThanOrEqual(12)
  })

  it('names icons that exist, at the size it says, including one that is maskable', () => {
    for (const icon of manifest.icons) {
      expect(existsSync(join(publicDir, icon.src))).toBe(true)
      expect(pngSize(join(publicDir, icon.src))).toBe(icon.sizes)
      expect(icon.type).toBe('image/png')
    }
    expect(manifest.icons.some((icon) => icon.sizes === '192x192' && (icon.purpose ?? 'any') === 'any')).toBe(true)
    expect(manifest.icons.some((icon) => icon.sizes === '512x512' && icon.purpose === 'any')).toBe(true)
    expect(manifest.icons.some((icon) => icon.purpose === 'maskable')).toBe(true)
  })

  it('has shortcuts inside its scope, and colors that are colors', () => {
    for (const shortcut of manifest.shortcuts) expect(shortcut.url.startsWith(manifest.scope)).toBe(true)
    expect(manifest.background_color).toMatch(/^#[0-9a-f]{6}$/i)
    expect(manifest.theme_color).toMatch(/^#[0-9a-f]{6}$/i)
  })

  it('is small files: nothing that a build has to wait for', () => {
    for (const file of ['manifest.webmanifest', 'sw.js', 'offline.html', 'offline.css']) expect(statSync(join(publicDir, file)).size).toBeLessThan(8_000)
    // The words of every kind of notice in both languages are in this one.
    expect(statSync(join(publicDir, 'push-notice.js')).size).toBeLessThan(12_000)
  })
})

describe('the page that links it', () => {
  const html = readFileSync(join(process.cwd(), 'index.html'), 'utf8')

  it('fetches the manifest with the viewer’s credentials, since the node asks for the Access sign-in on every request', () => {
    expect(html).toMatch(/<link rel="manifest" href="\/manifest\.webmanifest" crossorigin="use-credentials"/)
  })

  it('has an icon for the home screen and a theme color for each scheme', () => {
    expect(html).toMatch(/rel="apple-touch-icon" href="\/icons\/apple-touch-icon\.png"/)
    expect(html).toMatch(/name="theme-color" media="\(prefers-color-scheme: light\)"/)
    expect(html).toMatch(/name="theme-color" media="\(prefers-color-scheme: dark\)"/)
    expect(pngSize(join(publicDir, 'icons', 'apple-touch-icon.png'))).toBe('180x180')
  })
})
