import {
  MDXEditor,
  headingsPlugin,
  listsPlugin,
  quotePlugin,
  thematicBreakPlugin,
  linkPlugin,
  tablePlugin,
  codeBlockPlugin,
  codeMirrorPlugin,
  frontmatterPlugin,
} from "@mdxeditor/editor";
import "@mdxeditor/editor/style.css";

export function MarkdownPreview({
  content,
  compact,
}: {
  content: string;
  compact?: boolean;
}) {
  return (
    <div className={`mdx-preview-wrapper ${compact ? "compact" : ""}`}>
      <MDXEditor
        markdown={content}
        readOnly
        contentEditableClassName="mdx-preview-content"
        plugins={[
          headingsPlugin(),
          listsPlugin(),
          quotePlugin(),
          thematicBreakPlugin(),
          linkPlugin(),
          tablePlugin(),
          frontmatterPlugin(),
          codeBlockPlugin({ defaultCodeBlockLanguage: "" }),
          codeMirrorPlugin({ codeBlockLanguages: { "": "Plain", js: "JavaScript", ts: "TypeScript", lua: "Lua", python: "Python", rust: "Rust" } }),
        ]}
      />
    </div>
  );
}
