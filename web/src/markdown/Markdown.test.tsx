import { render, screen } from '@testing-library/react'
import { Markdown } from './Markdown'

describe('Markdown', () => {
  it('shows raw HTML as text instead of interpreting it', () => {
    const { container } = render(<Markdown source={'<img src="x" onerror="alert(1)">\n\n<script>alert(2)</script>'} />)
    expect(container.querySelector('img')).toBeNull()
    expect(container.querySelector('script')).toBeNull()
  })

  it('drops links with an unsafe scheme', () => {
    render(<Markdown source="[click](javascript:alert(1))" />)
    const link = screen.queryByRole('link', { name: 'click' })
    expect(link?.getAttribute('href') ?? '').not.toMatch(/^javascript:/i)
  })

  it('opens ordinary links in a new tab without leaking the opener', () => {
    render(<Markdown source="[docs](https://example.com/docs)" />)
    expect(screen.getByRole('link', { name: 'docs' })).toHaveAttribute('rel', 'noopener noreferrer')
    expect(screen.getByRole('link', { name: 'docs' })).toHaveAttribute('target', '_blank')
  })

  it('starts headings at level 3 so the document title stays the only level 2', () => {
    render(<Markdown source={'# One\n\n## Two'} />)
    expect(screen.getByRole('heading', { level: 3, name: 'One' })).toBeInTheDocument()
    expect(screen.getByRole('heading', { level: 4, name: 'Two' })).toBeInTheDocument()
    expect(screen.queryByRole('heading', { level: 1 })).toBeNull()
  })
})
