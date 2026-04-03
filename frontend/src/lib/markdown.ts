import { codeToHtml } from 'shiki';

function parseCodeLanguage(codeElement: Element): string {
  const languageClass = Array.from(codeElement.classList).find((value) =>
    value.startsWith('language-')
  );
  if (!languageClass) return 'text';
  const candidate = languageClass.slice('language-'.length).trim();
  return candidate.length > 0 ? candidate : 'text';
}

export async function highlightMarkdownCodeBlocks(html: string): Promise<string> {
  if (typeof window === 'undefined') return html;

  const document = new DOMParser().parseFromString(html, 'text/html');
  const codeBlocks = Array.from(document.querySelectorAll('pre > code'));

  for (const codeBlock of codeBlocks) {
    const pre = codeBlock.parentElement;
    if (!pre) continue;

    const language = parseCodeLanguage(codeBlock);
    const content = codeBlock.textContent ?? '';

    try {
      const highlighted = await codeToHtml(content, {
        lang: language,
        theme: 'github-dark'
      });

      const wrapper = document.createElement('div');
      wrapper.innerHTML = highlighted;
      const highlightedPre = wrapper.querySelector('pre');
      if (highlightedPre) {
        pre.replaceWith(highlightedPre);
      }
    } catch {
      // Keep the original block if highlighting fails.
    }
  }

  return document.body.innerHTML;
}
