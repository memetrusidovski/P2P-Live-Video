// Control panel for the one node the app runs. Every action goes through the
// Tauri commands in src-tauri/src/node.rs. The node serves the player page and
// the frame shows that page.
(() => {
  const { invoke } = window.__TAURI__.core;
  const $ = (id) => document.getElementById(id);
  const POLL_MS = 1000;
  let poll = null;
  let playerUrl = null;

  // RULE: Write to the DOM only when the value changed. A rewrite drops the
  //       user's text selection, which made the link impossible to copy.
  function setText(id, text) {
    const el = $(id);
    if (el.textContent !== text) el.textContent = text;
  }
  function setValue(id, value) {
    const el = $(id);
    if (el.value !== value) el.value = value;
  }

  function showError(message) {
    setText('error', message ? String(message) : '');
  }

  async function copy(id) {
    const input = $(id);
    const text = input.value;
    if (!text) return;
    input.focus();
    input.select();
    try {
      await navigator.clipboard.writeText(text);
    } catch (e) {
      // WHY: Older webviews expose no clipboard API; the selection plus the
      //      legacy copy command is the fallback.
      document.execCommand('copy');
    }
  }

  function render(s) {
    if (!s.role) {
      setText('status', 'no node running');
      $('stop').disabled = true;
      return;
    }
    const first = s.first_chunk_s != null ? `${s.first_chunk_s.toFixed(2)} s` : '-';
    setText('status',
      `${s.role} ${s.running ? '' : '(stopped) '}node ${s.node_id.slice(0, 12)}  stream ${s.stream_id.slice(0, 12)}` +
      `  state ${s.state || '-'}  played ${s.played}  starved ${s.starved}  layers ${s.layers}  first chunk ${first}`);
    setValue('share-link', s.share_link || '');
    setValue('player-url', s.player_url || '');
    $('copy-link').disabled = !s.share_link;
    $('copy-player').disabled = !s.player_url;
    $('stop').disabled = !s.running;
    showError(s.error);
    // Load the frame once per node; a reload would reconnect the player.
    if (s.running && s.player_url !== playerUrl) {
      playerUrl = s.player_url;
      $('player').src = playerUrl;
      $('player').hidden = false;
    }
    if (!s.running && playerUrl) {
      playerUrl = null;
      $('player').hidden = true;
      $('player').removeAttribute('src');
    }
  }

  async function refresh() {
    try {
      render(await invoke('status'));
    } catch (e) {
      showError(e);
    }
  }

  function startPolling() {
    if (!poll) poll = setInterval(refresh, POLL_MS);
  }

  async function run(command, args) {
    showError('');
    try {
      render(await invoke(command, args));
      startPolling();
    } catch (e) {
      showError(e);
    }
  }

  $('start-viewer').addEventListener('click', () =>
    run('start_viewer', { link: $('link').value, uploadKbps: Number($('view-kbps').value) }));
  $('start-publisher').addEventListener('click', () =>
    run('start_publisher', {
      path: $('path').value,
      uploadKbps: Number($('pub-kbps').value),
      ingest: $('ingest').value,
      renditions: $('renditions').value,
    }));
  $('ingest').addEventListener('change', () => {
    $('renditions').disabled = $('ingest').value !== 'video';
  });
  $('stop').addEventListener('click', () => run('stop'));
  $('copy-link').addEventListener('click', () => copy('share-link'));
  $('copy-player').addEventListener('click', () => copy('player-url'));

  refresh();
})();
