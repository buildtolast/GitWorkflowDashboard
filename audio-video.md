# Audio/Video Presentation Checklist

A checklist for producing clean, synced audio/video presentations with AI-generated narration.

---

## 1. Script Preparation

- [ ] Write narration script with `[SLIDE N]` and `[DEMO]` markers for each visual change
- [ ] Include transitional phrases between sections ("Next, let's talk about...", "Moving on to...")
- [ ] Add `[pause 1.5s]` markers between sections for natural breathing room
- [ ] Check character count against TTS provider limits (ElevenLabs: 5000 chars max)
- [ ] Replace special characters that cause TTS issues:
  - Em-dashes `—` → ` - ` (prevents garbled output like `,Äì`)
  - Curly quotes → straight quotes
  - Non-ASCII punctuation → ASCII equivalents
- [ ] Create a clean narration-only text file (strip all markers/timing notes) for TTS upload
- [ ] Spell out numbers as words for natural speech ("twenty-three" not "23")

## 2. Audio Generation

- [ ] Choose voice and model (e.g., ElevenLabs Josh - Teacher for Kids, Eleven Multilingual v2)
- [ ] Generate audio and note exact duration
- [ ] Run silence detection to find actual speech boundaries:
  ```bash
  ffmpeg -i audio.mp3 -af silencedetect=noise=-30dB:d=0.8 -f null - 2>&1 | grep silence
  ```
- [ ] Map each `silence_end` timestamp to the corresponding slide/demo transition
- [ ] Cross-reference silence boundaries with script section word counts for validation
- [ ] Listen to the full audio once to catch pronunciation issues before building video

## 3. Screenshot Capture

### Slide Screenshots
- [ ] Use headless Chrome for slide captures (no browser chrome):
  ```bash
  chrome --headless --screenshot --window-size=1920,1080 "file://slides.html?slide=N"
  ```
- [ ] Verify all slides are 1920x1080 PNG

### Demo Screenshots (Live App)
- [ ] **Never use `screencapture` for app demos** — it captures browser chrome (tabs, address bar, bookmarks, debug banners)
- [ ] Instead, use one of these chrome-free methods:
  - **Chrome DevTools Protocol** screenshot (captures viewport only)
  - **Chrome MCP tools** screenshot (captures tab content only)
  - **Headless Chrome** with `--screenshot` flag
- [ ] If you must use `screencapture`:
  - Activate the correct window first: `osascript -e 'tell application "Google Chrome" to activate'`
  - Wait 0.5s for window to fully render
  - Account for Retina scaling (multiply logical pixels by 2 for crop values)
  - Crop out ALL browser chrome: tab bar, address bar, bookmarks bar, debug banners
  - Test crop values carefully — verify the dashboard header is preserved
- [ ] Verify theme consistency across all demo screenshots (dark/light)
- [ ] Resize all demo screenshots to exactly 1920x1080:
  ```bash
  sips -z 1080 1920 screenshot.png
  ```
- [ ] Verify no sensitive data (tokens, passwords, personal info) visible in screenshots

## 4. Slide Timing & Sync

**This is the most critical step. Getting this wrong makes the entire video feel broken.**

- [ ] **Rule: Each slide/image must appear AT or BEFORE its narration starts, never after**
- [ ] Use silence detection timestamps as transition points (not estimated word counts)
- [ ] For each section, set slide duration = `next_section_silence_end - current_section_silence_end`
- [ ] Add 0.5-1.0s buffer before each narration starts (slide appears slightly early)
- [ ] Account for fade transition duration (typically 0.5s) — the fade eats into visible time
- [ ] Total of all slide durations must equal the audio duration exactly
- [ ] Double-check the "listing" sections (e.g., "Six Tabs") where rapid-fire items can desync
- [ ] Write out the full timing table before building:
  ```
  Section          | Start  | Duration | Image
  Title            | 0.0s   | 10.5s    | slide_1.png
  Problem          | 10.5s  | 16.9s    | slide_2.png
  ...
  ```

## 5. Video Assembly (ffmpeg)

- [ ] Use this proven ffmpeg pattern:
  ```bash
  ffmpeg -y \
    -loop 1 -t <duration1> -i slide_1.png \
    -loop 1 -t <duration2> -i slide_2.png \
    ... \
    -i audio.mp3 \
    -filter_complex "
      [0:v]scale=1920:1080,format=yuv420p,fade=t=out:st=<dur1-0.5>:d=0.5[v0];
      [1:v]scale=1920:1080,format=yuv420p,fade=t=in:st=0:d=0.5,fade=t=out:st=<dur2-0.5>:d=0.5[v1];
      ...
      [v0][v1]...[vN]concat=n=N:v=1:a=0[outv]
    " \
    -map "[outv]" -map <N>:a \
    -c:v libx264 -preset medium -crf 18 -r 30 \
    -c:a aac -b:a 192k \
    -shortest -movflags +faststart output.mp4
  ```
- [ ] Fade durations: 0.5s in, 0.5s out (consistent across all slides)
- [ ] `fade=t=out:st=` value = slide duration minus fade duration
- [ ] First slide: fade out only. Last slide: fade in only. Middle slides: both.
- [ ] Use `-shortest` to avoid black frames if durations don't sum perfectly
- [ ] Use `-movflags +faststart` for web-compatible seeking

## 6. Quality Verification

- [ ] Watch the complete video end-to-end before sharing
- [ ] Check sync at these critical points:
  - [ ] First slide transition (sets the tone)
  - [ ] Any "listing" section (multiple items read quickly)
  - [ ] Demo sections (slide should show feature BEFORE it's described)
  - [ ] Last slide (should not cut off or have trailing silence)
- [ ] Verify no browser chrome visible in any demo frame
- [ ] Verify consistent theme across all frames
- [ ] Check video metadata: 1920x1080, H.264, AAC audio, reasonable file size
- [ ] Test playback in at least one player (QuickTime, VLC, or browser)

## 7. Common Pitfalls

| Pitfall | Cause | Fix |
|---------|-------|-----|
| Audio ahead of slides | Slide durations estimated, not measured | Use silence detection timestamps |
| Browser chrome in demos | Used `screencapture` without cropping | Use headless/DevTools screenshots |
| Garbled TTS characters | Special Unicode in script (em-dash, etc.) | Replace with ASCII equivalents |
| TTS too long/short | Character count exceeded or script too sparse | Check limits, adjust script density |
| Theme inconsistency | App state changed between captures | Set theme explicitly before each capture |
| Retina scaling surprise | macOS captures at 2x resolution | Multiply all crop coords by 2 |
| Wrong window captured | Terminal/editor was in front of Chrome | `osascript activate` + sleep before capture |
| Fade eats content | Fade duration not subtracted from visible time | Set fade start = duration - fade_length |
| Black frames at end | Slide durations don't sum to audio length | Use `-shortest` flag |

---

## Quick Reference

```
# Silence detection
ffmpeg -i audio.mp3 -af silencedetect=noise=-30dB:d=0.8 -f null - 2>&1 | grep silence

# Headless Chrome screenshot (no browser chrome)
chrome --headless --screenshot=slide.png --window-size=1920,1080 "URL"

# Resize to 1920x1080
sips -z 1080 1920 image.png

# Check image dimensions
sips -g pixelWidth -g pixelHeight image.png

# Crop with ffmpeg (Retina: multiply logical pixels by 2)
ffmpeg -i input.png -vf "crop=3840:2160:0:560" output.png
```
