(() => {
  return Promise.resolve("dat2wasm2img.wasm")
    .then((url) => fetch(url))
    .then((res) => WebAssembly.instantiateStreaming(res))
    .then(async (pair) => {
      const res = await fetch("img.exr");
      const exr = await res?.arrayBuffer();
      const {
        module,
        instance,
      } = pair || {};
      const {
        memory,
        exr_input_ptr,
        exr_emsg_ptr,
        exr_msg_sz,
        exr_reset,
        y2image32f,
        exr_width,
        exr_height,
        exr_ptr,
      } = instance?.exports || {};
      const exrSize = exr?.byteLength;
      const sz = exr_reset(exrSize);
      const ofst = exr_input_ptr();
      const buffer = memory?.buffer ?? new ArrayBuffer(0);
      const typed = new Uint8Array(buffer, ofst, exrSize);
      const eview = new Uint8Array(exr);
      for (let i = 0; i < exr.byteLength; i++) {
        typed[i] = eview[i];
      }
      const decodedSize = y2image32f(exrSize);
      const dec = new TextDecoder();
      const emsg = new Uint8Array(
        buffer,
        exr_emsg_ptr(),
        exr_msg_sz(),
      );
      const decoded = dec.decode(emsg);
      if (decoded) console.info("error: " + decoded);
      const width = exr_width();
      const height = exr_height();
      const f32arr = new Float32Array(
        buffer,
        exr_ptr(),
        decodedSize,
      );
	  const root = document.getElementById("root")
      const meta = {
        decodedSize,
        width,
        height,
      };
	  root.textContent = JSON.stringify(meta)
	  return f32arr
    })
    .then(console.info)
    .catch(console.warn);
})();
