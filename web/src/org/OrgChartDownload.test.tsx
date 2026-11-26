import { screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderApp } from '../test/render'

// Some browsers (WebKit among them) refuse to read back a canvas that drew an SVG with a foreignObject, so
// the PNG can fail where the SVG works. The page must say so, point to the SVG, and give the buttons back.
vi.mock('./exportImage', async (original) => ({
  ...(await original<typeof import('./exportImage')>()),
  svgToPng: vi.fn(() => Promise.reject(new Error('The canvas cannot be read.'))),
}))

describe('a PNG the browser cannot make', () => {
  it('says why, offers the SVG, and enables the buttons again', async () => {
    const user = userEvent.setup()
    renderApp({ path: '/org' })
    const png = await screen.findByRole('button', { name: 'PNG' })
    await user.click(png)
    const toast = await screen.findByText(/Could not make the PNG: The canvas cannot be read\. Try the SVG, and open it in a browser\./)
    expect(toast.closest('.toast')).toHaveClass('error')
    await waitFor(() => expect(screen.getByRole('button', { name: 'PNG' })).toBeEnabled())
    expect(screen.getByRole('button', { name: 'SVG' })).toBeEnabled()
  })

  it('does not call a failed SVG a PNG, and does not offer itself', async () => {
    const exportImage = await import('./exportImage')
    const spy = vi.spyOn(exportImage, 'buildChartSvg').mockImplementation(() => {
      throw new Error('No size.')
    })
    try {
      const user = userEvent.setup()
      renderApp({ path: '/org' })
      await user.click(await screen.findByRole('button', { name: 'SVG' }))
      const toast = await screen.findByText(/Could not make the SVG: No size\./)
      expect(toast).not.toHaveTextContent('Try the SVG')
    } finally {
      spy.mockRestore()
    }
  })
})
