import {
  MDXEditor,
  headingsPlugin,
  listsPlugin,
  quotePlugin,
  thematicBreakPlugin,
  markdownShortcutPlugin,
  linkPlugin,
  linkDialogPlugin,
  tablePlugin,
  codeBlockPlugin,
  codeMirrorPlugin,
  frontmatterPlugin,
} from "@mdxeditor/editor";
import "@mdxeditor/editor/style.css";
import { useCallback, useEffect, useRef } from "react";
import type { MDXEditorMethods } from "@mdxeditor/editor";

export function MarkdownEditor({
  initialValue,
  onChange,
}: {
  initialValue: string;
  onChange?: (value: string) => void;
}) {
  const editorRef = useRef<MDXEditorMethods>(null);
  // Only forward onChange after the user has actually typed/pasted in the editor.
  // MDXEditor normalizes markdown on init/setMarkdown which triggers onChange with
  // content that differs from the original — we must ignore those spurious events.
  const userEditedRef = useRef(false);

  // Reset flag when content is externally replaced (file switch, refresh, etc.)
  useEffect(() => {
    userEditedRef.current = false;
    if (editorRef.current) {
      editorRef.current.setMarkdown(initialValue);
    }
  }, [initialValue]);

  const handleChange = useCallback(
    (val: string) => {
      if (!userEditedRef.current) return;
      onChange?.(val);
    },
    [onChange],
  );

  // onInput fires only on real user input (typing, pasting, IME) in contenteditable,
  // NOT on programmatic DOM updates from ProseMirror/MDXEditor.
  const handleInput = useCallback(() => {
    userEditedRef.current = true;
  }, []);

  return (
    <div className="mdx-editor-wrapper" onInputCapture={handleInput}>
      <MDXEditor
        ref={editorRef}
        markdown={initialValue}
        onChange={handleChange}
        contentEditableClassName="mdx-editor-content"
        plugins={[
          headingsPlugin(),
          listsPlugin(),
          quotePlugin(),
          thematicBreakPlugin(),
          markdownShortcutPlugin(),
          linkPlugin(),
          linkDialogPlugin(),
          tablePlugin(),
          frontmatterPlugin(),
          codeBlockPlugin({ defaultCodeBlockLanguage: "" }),
          codeMirrorPlugin({ codeBlockLanguages: { "": "Plain", js: "JavaScript", ts: "TypeScript", lua: "Lua", python: "Python", rust: "Rust" } }),
        ]}
      />
    </div>
  );
}
