import ReactMarkdown, { type Components } from 'react-markdown'
import remarkGfm from 'remark-gfm'
import './markdown.css'

/** The document title is an h2, so the body's own headings start at h3 and never repeat it. */
const DEMOTED: Components = {
  h1: ({ node: _node, ...props }) => <h3 {...props} />,
  h2: ({ node: _node, ...props }) => <h4 {...props} />,
  h3: ({ node: _node, ...props }) => <h5 {...props} />,
  h4: ({ node: _node, ...props }) => <h6 {...props} />,
  h5: ({ node: _node, ...props }) => <h6 {...props} />,
  h6: ({ node: _node, ...props }) => <h6 {...props} />,
}

/**
 * Renders a proposal body. Raw HTML in the source is never interpreted (react-markdown escapes it
 * and no HTML plugin is installed), and unsafe link schemes such as `javascript:` are dropped.
 */
export function Markdown({ source }: { source: string }) {
  return (
    <div className="md">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
          ...DEMOTED,
          a: ({ node: _node, ...props }) => <a {...props} target="_blank" rel="noopener noreferrer" />,
          table: ({ node: _node, ...props }) => (
            <div className="md-table">
              <table {...props} />
            </div>
          ),
        }}
      >
        {source}
      </ReactMarkdown>
    </div>
  )
}
