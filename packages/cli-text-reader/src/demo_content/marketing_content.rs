// Demo content for marketing demonstration

pub fn get_marketing_demo_content() -> String {
  r#"# The Art of Reading in the Digital Age

In an era where information flows ceaselessly through countless channels, the simple act of reading has become both more crucial and more challenging than ever before. We find ourselves navigating between the need for deep, focused comprehension and the relentless pull of digital distractions.

The modern reader faces a paradox: while we have unprecedented access to the world's knowledge, our tools often work against our ability to truly absorb and understand what we read. Notifications interrupt our flow, advertisements compete for our attention, and interfaces designed for engagement rather than comprehension fragment our focus into ever-smaller pieces.

The modern reader faces a paradox: while we have unprecedented access to the world's knowledge, our tools often work against our ability to truly absorb and understand what we read. Notifications interrupt our flow, advertisements compete for our attention, and interfaces designed for engagement rather than comprehension fragment our focus into ever-smaller pieces.

Yet within this chaos, a movement toward intentional, mindful reading has emerged. Readers are rediscovering the power of simplicity, seeking tools that enhance rather than hinder their connection with text. They understand that true comprehension requires not just access to information, but an environment that respects the cognitive demands of deep reading.

## The Evolution of Text Display

The journey from physical to digital text represents one of humanity's most profound technological shifts. What began as crude phosphorescent characters on monochrome monitors has evolved into a sophisticated ecosystem of reading technologies. Modern terminal-based readers, paradoxically, represent both a return to simplicity and an embrace of advanced text processing capabilities. They strip away the visual noise of contemporary interfaces while leveraging decades of refinement in text rendering, navigation paradigms, and user interaction models. This fusion of minimalism and power creates an environment where focus becomes the default state, where reading transforms from passive consumption to active engagement, and where the boundaries between reader and text dissolve into a seamless flow of information and understanding.




### Terminal-Based Reading

Despite the availability of graphical interfaces, terminal-based reading applications have maintained a dedicated following. The reasons include:

- **Minimal distractions**: No ads, popups, or visual clutter
- **Keyboard efficiency**: Navigate without reaching for the mouse
- **Resource efficiency**: Low CPU and memory usage
- **Universal compatibility**: Works on any system with a terminal
- **Customization**: Complete control over appearance and behavior

## Navigation Paradigms

Different reading applications employ various navigation paradigms, each with its own philosophy and target audience. The most influential of these paradigms comes from the vi/vim text editor tradition.

### The Vi Philosophy

Vi's modal editing approach, where different modes serve different purposes, translates beautifully to reading applications. In normal mode, every key becomes a navigation command, allowing for incredibly efficient movement through text without modifier keys.

Common navigation patterns include:

- **Character movement**: h, j, k, l for left, down, up, right
- **Word movement**: w (next word), b (previous word), e (end of word)
- **Line movement**: 0 (start), $ (end), ^ (first non-blank)
- **Screen movement**: H (top), M (middle), L (bottom)
- **Document movement**: gg (beginning), G (end), Ctrl-f/b (page)

## Document Formats and Processing

Modern document readers must handle a variety of formats, each with its own structure and rendering requirements. The most common formats include:

### PDF (Portable Document Format)

PDFs preserve exact layout and formatting, making them ideal for documents that need to maintain their appearance across different systems. However, extracting text from PDFs can be challenging due to:

- Complex layout structures
- Embedded fonts and encodings
- Multi-column layouts
- Headers, footers, and page numbers
- Images and diagrams interspersed with text

### EPUB (Electronic Publication)

EPUB files are essentially ZIP archives containing HTML, CSS, and metadata. This format is designed specifically for e-readers and offers:

- Reflowable text that adapts to screen size
- Support for multiple chapters and navigation
- Embedded fonts and styles
- Metadata including author, title, and cover images

## Text Processing Pipeline

Converting documents for terminal display involves several stages:

1. **Extraction**: Pulling raw text from the source format
2. **Cleaning**: Removing artifacts and normalizing whitespace
3. **Formatting**: Applying word wrapping and justification
4. **Enhancement**: Adding navigation aids and bookmarks

### Text Justification

Proper text justification improves readability by creating even margins and consistent spacing. The algorithm must balance several factors:

- Minimizing rivers of whitespace
- Avoiding excessive letter spacing
- Handling hyphenation appropriately
- Preserving paragraph structure

## Reading Comprehension and Speed

Research has shown that reading speed and comprehension are influenced by numerous factors, including line length, font choice, and contrast ratio. Terminal-based readers can optimize these factors for individual preferences.

### Optimal Line Length

Studies suggest that line lengths between 50-75 characters provide the best balance between reading speed and comprehension. Shorter lines require more eye movements, while longer lines make it difficult to track from one line to the next.

## Future Directions

As technology continues to evolve, terminal-based readers are incorporating new features while maintaining their core simplicity:

- **Cloud synchronization**: Seamless reading across devices
- **AI-powered summaries**: Quick document overview
- **Voice integration**: Text-to-speech capabilities
- **Collaborative annotations**: Shared notes and highlights
- **Advanced search**: Semantic and fuzzy matching

The future of digital reading lies not in replacing traditional methods, but in providing readers with choices that match their preferences and workflows. Whether you prefer a feature-rich GUI application or a minimal terminal interface, the goal remains the same: efficient, enjoyable access to the world's written knowledge.

## Conclusion

Terminal-based document readers represent a unique approach to digital reading that prioritizes efficiency, minimalism, and keyboard-driven navigation. By embracing the constraints of the terminal environment, these applications offer a focused, distraction-free reading experience that many users find more productive than their graphical counterparts.

As we continue to navigate the digital transformation of reading, tools that respect user preferences and provide genuine value will endure. The terminal may seem like an anachronism in our graphical age, but for those who value efficiency and focus, it remains an ideal reading environment."#.to_string()
}
