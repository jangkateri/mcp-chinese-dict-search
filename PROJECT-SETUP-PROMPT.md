# PROJECT-SETUP-PROMPT
This project repo is about creating an MCP server for searching a dictionary zip file in the supported Markdown layout. The reference corpus used during development is a Chinese medicine dictionary; it is user-supplied and never committed. The content may contain an images folder with .jpg files referenced by the .md files for illustration. In the reference layout the start file is 08-a.md for entries beginning with the letter `a`, continuing 09-b.md up to 33-z.md . The entry key inside the dictionary is tag with `\*` and the value associated with the dictionary key entry content is up to the next `\*`. Inside the value content can have a <a href>...</a> (e.g. <a href="10-c.md#a1PUT" class="class_s1NTM-1">cold</a>) containing a reference to another .md file with the name refer to by the href. If you check the example above the href said `10-c.md#a1PUT`, this refer to `10-c.md` file with an entry key of `a1PUT` .
There are four other appendix then refer to Acupuncture Meridian, Herbal Medicine Agent/Drug, Hermal Formulas and Weight & Measurement dosage standards.  Originally, the files for this appendix are name 34-appendix-1.md up to 37-appendix-4.md, I explicitly added the topic of the Appendix, Meridian, Medicinal, Formulas and Weights-Measurements .

## Structure
This project intention was to create an MCP Server type stdio, intended to be launch in Claude Code, OpenCode and or Deepseek Harness.
The structure of the project should follow a reference Rust MCP project.

## Tool
What this mcp server will provide are functionality of doing a search for term useds in Chinese Medicine, (English Term).

If the term refer to Meridian, or Acupuncture point name or Standard organ meridian point number or point name, additional information will also search the 34-appendix-1-Meridian.md. Same rule will apply for the Herbal medicinal, Herbal Formulas and Weight & Measurement appendix.

## Enhancement (eventually)
rust tools will be created to generate dictionary using PinYin as the key and value content will be the English key entry tag to redirect the PinYin to get the content from the given .md files. Another dictionary will also be generate to have the Simplified or Traditional Chinese text as the key and value content will be the English key entry tag.
The MCP server codes will be updated to provide the enhancement tool for this.

## Extra Information
A file name DictionarySetupMCP.txt is the result of using Gemini to originally asking that AI to provide ideas about this project. This is an extra information about my interaction with that AI LLM, not necessarily for this implementation. Take it just a reminder that eventually I might extend this project to include other books in Chinese Medicine that can be used to do research searching and I am looking for either a RAG functionality or creating a Small Language Model to interact with.

## Create CLAUDE.md or AGENT.md with the following added content
---

### Behavior Guidelines
- Always be precise and technical.
- Do not speculate; if unsure, state what information is missing.
- Follow code style guidelines exactly.
- If you know code is buggy, you should not perform a git commit.
- CRITICAL: DO NOT USE sed tool, IT CORRUPTS FILES in Windows OS Environment.
---

### CRITICAL: RESEARCH BEFORE GIVING UP

**When encountering a limitation or error, do NOT assume it's a dead end without thorough research.**

#### Required Research Steps

1. **USE WEB SEARCH** - Search for solutions, workarounds, and similar problems
   - Search Office.js documentation
   - Search Stack Overflow, GitHub issues
   - Search for "Office.js workaround" or "Office.js limitation workaround"

2. **THINK CREATIVELY** - Consider alternative approaches:
   - Can the problem be solved differently?
   - Are there workarounds using different APIs?
   - Can the data flow be restructured?

3. **VERIFY LIMITATIONS** - Don't assume something is impossible without proof:
   - Document what you tried
   - Document what error you received
   - Only call it a limitation after exhaustive research

4. **PROPOSE WORKAROUNDS** - Even if direct solution isn't possible:
   - Can intermediate steps solve it?
   - Can helper cells/functions help?
   - Can the approach be inverted?

#### Example of What NOT To Do
```
❌ "This is an Office.js limitation" (without research)
✅ "Let me search for workarounds to this issue"
✅ "I found several approaches others have used..."
```

#### Before Declaring a Limitation

You MUST:
1. Search the web for solutions
2. Try at least 2-3 different approaches
3. Document what was attempted
4. Only then propose a workaround

---

### PROBLEM SOLVING GUIDELINES

#### When Stuck on a Bug or Issue

**DO NOT GIVE UP EASILY.** Follow these steps before accepting a limitation:

1. **Research First** - Use web search to find if others have solved the same problem
   - Search for the specific error or API limitation
   - Check Stack Overflow, Microsoft Docs, GitHub issues
   - Look for workarounds others have used

2. **Think Creatively** - Consider alternative approaches:
   - Can the problem be solved differently?
   - Is there a workaround using different APIs?
   - Can we transform the input to make it work?

3. **Test Assumptions** - Don't assume something is a limitation without proof
   - Try simple test cases to isolate the issue
   - Verify the exact error message
   - Check if it's a caching or configuration issue

4. **Document What Was Tried** - Even if you can't solve it, document:
   - What approaches were attempted
   - What the error messages were
   - What might work as a solution

#### Example of NOT Giving Up

**Bad Response:** "This is an Office.js limitation, it can't be done."
**Good Response:** "The direct approach doesn't work, but let me try using helper formulas as a workaround."

---

### Extra content

Include andrej-karpathy-skills to the CLAUDE.md or AGENT.md file to provide coding guideline and LLM interaction.

---
