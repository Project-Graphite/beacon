import type { ReactNode } from 'react';
import { parseMarkdown, type Block, type Inline } from '../lib/markdown.ts';

function inlineNodes(nodes: Inline[]): ReactNode[] {
  return nodes.map((node, index) => {
    switch (node.type) {
      case 'text':
        return node.text;
      case 'code':
        return (
          <code key={index} className="rounded-[4px] bg-surface px-1 font-mono text-micro">
            {node.text}
          </code>
        );
      case 'strong':
        return (
          <strong key={index} className="font-semibold">
            {inlineNodes(node.children)}
          </strong>
        );
      case 'emphasis':
        return <em key={index}>{inlineNodes(node.children)}</em>;
      case 'link':
        return (
          <span key={index}>
            {inlineNodes(node.children)} <span className="break-all text-faint">{node.url}</span>
          </span>
        );
    }
  });
}

function blockNode(block: Block, key: number): ReactNode {
  switch (block.type) {
    case 'heading':
      return (
        <p key={key} className="text-body font-semibold">
          {inlineNodes(block.children)}
        </p>
      );
    case 'paragraph':
      return (
        <p key={key} className="whitespace-pre-line">
          {inlineNodes(block.children)}
        </p>
      );
    case 'code':
      return (
        <pre key={key} className="rounded-field bg-surface px-2 py-1.5 font-mono text-micro whitespace-pre-wrap [overflow-wrap:anywhere]">
          {block.text}
        </pre>
      );
    case 'list': {
      const List = block.ordered ? 'ol' : 'ul';
      return (
        <List
          key={key}
          start={block.ordered ? block.start : undefined}
          className={`mt-0.5 space-y-0.5 pl-5 marker:text-muted ${block.ordered ? 'list-decimal' : 'list-disc'}`}
        >
          {block.items.map((item, index) => (
            <li key={index} className="whitespace-pre-line">
              {inlineNodes(item.children)}
              {item.nested && blockNode(item.nested, 0)}
            </li>
          ))}
        </List>
      );
    }
  }
}

export function Markdown({ source }: { source: string }) {
  return (
    <div className="space-y-2 text-secondary text-ink [overflow-wrap:anywhere]">
      {parseMarkdown(source).map(blockNode)}
    </div>
  );
}
