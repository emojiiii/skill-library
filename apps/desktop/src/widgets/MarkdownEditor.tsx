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
  onDirtyChange,
}: {
  initialValue: string;
  onChange?: (value: string) => void;
  /**
   * Reports whether the editor content differs from its own normalized
   * baseline. This is the correct dirty signal: MDXEditor re-serializes
   * markdown on mount, so comparing its output against the original GitHub
   * text always looks "changed". Instead we capture the first normalized value
   * as the baseline and compare against that — editing then reverting returns
   * to the baseline and clears dirty.
   */
  onDirtyChange?: (dirty: boolean) => void;
}) {
  const editorRef = useRef<MDXEditorMethods>(null);
  // Only forward onChange after the user has actually typed/pasted in the editor.
  // MDXEditor normalizes markdown on init/setMarkdown which triggers onChange with
  // content that differs from the original — we must ignore those spurious events.
  const userEditedRef = useRef(false);
  // The editor's own normalized rendering of the current initialValue. Set on
  // the first onChange after (re)load; dirty is measured against this.
  const baselineRef = useRef<string | null>(null);

  // Reset flags when content is externally replaced (file switch, refresh, etc.)
  useEffect(() => {
    userEditedRef.current = false;
    baselineRef.current = null;
    onDirtyChange?.(false);
    if (editorRef.current) {
      editorRef.current.setMarkdown(initialValue);
    }
  }, [initialValue, onDirtyChange]);

  const handleChange = useCallback(
    (val: string) => {
      // Capture the editor's normalized baseline on the first change event
      // (fired by the initial setMarkdown), before the user has edited.
      if (baselineRef.current === null) {
        baselineRef.current = val;
      }
      if (!userEditedRef.current) return;
      onChange?.(val);
      onDirtyChange?.(val !== baselineRef.current);
    },
    [onChange, onDirtyChange],
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
