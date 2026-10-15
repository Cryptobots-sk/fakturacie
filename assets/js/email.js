const $ = (selector, root=document) => root.querySelector(selector);
const $$ = (selector, root=document) => [...root.querySelectorAll(selector)];
const values = form => Object.fromEntries(new FormData(form));
const requestKey = () => [...crypto.getRandomValues(new Uint8Array(16))].map(value=>value.toString(16).padStart(2,'0')).join('');

export function initEmail({boot,api,notify,errorAt,fillForm}) {
  if(boot.page==='settings') initSmtp({boot,api,notify,errorAt,fillForm});
  if(boot.page==='editor' && $('#open-email')) initComposer({boot,api,errorAt,fillForm});
}
function initSmtp({boot,api,notify,errorAt,fillForm}) {
  let smtp=boot.smtp, testKey=boot.smtpTestKey, saving=false;
  const form=$('#smtp-form'), testForm=$('#smtp-test-form'), testButton=$('#smtp-test-button');
  function render() {
    fillForm(form,smtp); $('#smtp-password').value=''; $('#smtp-clear-password').checked=false;
    $('#smtp-password').placeholder=smtp.passwordSet?'Heslo je uložené':'';
    $('#smtp-password-hint').textContent='Uložené heslo sa nezobrazuje. Prázdne pole ponechá pôvodné heslo. Pri zmene servera alebo používateľa zadaj heslo znova.';
    testButton.disabled=!smtp.configured;
    testButton.textContent='Poslať skúšobný e-mail';
    $('#smtp-test-hint').textContent=smtp.configured?'Odošle sa cez uložené SMTP nastavenia.':'Najprv ulož SMTP nastavenia.';
  }
  render();
  $('#smtp-test-to').value=smtp.fromEmail || boot.supplier.email || '';
  if(!smtp.fromName) $('#smtp-fromName').value=boot.supplier.name;
  if(!smtp.fromEmail) $('#smtp-fromEmail').value=boot.supplier.email || '';
  form.addEventListener('input',()=>{testButton.disabled=true;$('#smtp-test-hint').textContent='Najprv ulož zmenené SMTP nastavenia.';});
  form.addEventListener('submit',async event=>{
    event.preventDefault(); if(saving) return; saving=true;
    const button=$('button[type=submit]',form);button.disabled=true;
    try {
      smtp=await api('smtpSave',{...values(form),port:Number($('#smtp-port').value),version:smtp.version,clearPassword:$('#smtp-clear-password').checked});
      testKey=requestKey();
      render();errorAt('#smtp-error','');notify('SMTP nastavenia sú uložené.');
    } catch(error) {errorAt('#smtp-error',error.message);}
    finally {saving=false;button.disabled=false;}
  });
  testForm.addEventListener('submit',async event=>{
    event.preventDefault();testButton.disabled=true;const status=$('#smtp-test-status');status.hidden=false;status.textContent='Odosielam skúšobný e-mail…';
    try {
      const result=await api('smtpTest',{to:$('#smtp-test-to').value,smtpVersion:smtp.version,requestKey:testKey});
      status.textContent=result.message;
      // Keep the same key when a request acknowledgement is lost.
      if(result.status==='sent') testButton.textContent='Skúška odoslaná';
    } catch(error) {status.textContent=error.message;testButton.disabled=false;}
  });
}
function initComposer({boot,api,errorAt,fillForm}) {
  const dialog=$('#email-dialog'), form=$('#email-form'), open=$('#open-email'), send=$('#email-send');
  let draft=null, busy=false, attempted=false;
  function frozen(value) {$$('input,textarea',form).forEach(input=>input.disabled=value);send.disabled=value;}
  function history(result) {
    $('#email-history-empty')?.remove();
    $(`[data-delivery-id="${result.id}"]`)?.remove();
    const row=document.createElement('div');row.className='list-group-item';
    row.dataset.deliveryId=result.id;
    const label=document.createElement('p');label.className='mb-1 fw-semibold';label.textContent=result.message;
    const details=document.createElement('p');details.className='small mb-0 text-secondary';details.textContent=`${result.recipient} · ${draft.subject}`;
    row.append(label,details);$('#email-history').prepend(row);
  }
  open.addEventListener('click',async()=>{
    open.disabled=true;
    try {
      draft=await api('invoiceEmailPreview',{id:boot.invoice.id,version:boot.invoice.version}); attempted=false;frozen(false);
      fillForm(form,draft);errorAt('#email-error','');$('#email-status').hidden=true;
      $('#email-config-hint').hidden=draft.configured;send.disabled=!draft.configured;
      $('#email-from').textContent=`Odosielateľ: ${draft.fromEmail || 'SMTP zatiaľ nie je nastavené'}`;
      for(const field of ['number','customer','amount','dueDate','iban','bic','attachment']) $(`#email-${field}`).textContent=draft[field] || '—';
      $('#email-reference').textContent=draft.number;$('#email-message-preview').textContent=draft.message;
      $('#email-qrs').replaceChildren(...draft.qrImages.map(image=>{
        const col=document.createElement('div');col.className='col-12 col-sm-6 col-md-4';
        const img=document.createElement('img');img.src=image.dataUri;img.alt=`Platobný QR: ${image.label}`;img.width=180;img.height=180;img.className='img-fluid';
        const label=document.createElement('p');label.className='small mt-2 mb-0';label.textContent=image.label;col.append(img,label);return col;
      }));
      dialog.showModal();$('#email-to').focus();
    } catch(error) {errorAt('#editor-error',error.message);}
    finally {open.disabled=false;}
  });
  $('#email-message').addEventListener('input',event=>$('#email-message-preview').textContent=event.target.value);
  dialog.addEventListener('cancel',event=>{if(busy) event.preventDefault();});
  form.addEventListener('submit',async event=>{
    event.preventDefault();if(busy || !draft.configured) return;
    if(!attempted) draft={...draft,...values(form)};
    attempted=true;busy=true;frozen(true);$$('[data-close-dialog]',dialog).forEach(button=>button.disabled=true);
    const status=$('#email-status');status.hidden=false;status.textContent='Odosielam e-mail…';errorAt('#email-error','');
    try {
      const result=await api('invoiceEmailSend',{id:boot.invoice.id,version:draft.invoiceVersion,smtpVersion:draft.smtpVersion,requestKey:draft.requestKey,to:draft.to,subject:draft.subject,message:draft.message});
      status.textContent=result.message;history(result);
    } catch(error) {errorAt('#email-error',error.message);status.textContent='Opakované stlačenie overí tú istú požiadavku bez opätovného odoslania.';send.disabled=false;}
    finally {busy=false;$$('[data-close-dialog]',dialog).forEach(button=>button.disabled=false);}
  });
}
