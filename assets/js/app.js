import Dropdown from 'bootstrap/js/dist/dropdown';
import {initEmail} from './email.js';
import {calculate, money, decimalUnits} from './money.mjs';

const $ = (selector, root = document) => root.querySelector(selector);
const $$ = (selector, root = document) => [...root.querySelectorAll(selector)];
const boot = JSON.parse($('#app-data').textContent);
const csrf = $('meta[name="csrf-token"]').content;
const normalize = text => String(text).normalize('NFD').replace(/\p{Diacritic}/gu, '').toLocaleLowerCase('sk');
let toastTimer;
function notify(message) {
  const box = $('#notification');
  box.textContent = message; box.hidden = false;
  clearTimeout(toastTimer); toastTimer = setTimeout(() => {box.hidden = true;}, 6000);
}
function errorAt(selector, message) {
  const element = $(selector); element.textContent = message || ''; element.hidden = !message;
}
async function api(action, data) {
  let response;
  try {
    const route = action.replace(/[A-Z]/g, letter => `-${letter.toLowerCase()}`);
    response = await fetch(`/api/${route}`, {method:'POST', headers:{'Content-Type':'application/json','X-CSRF-Token':csrf}, body:JSON.stringify(data)});
  } catch {
    throw new Error('Spojenie sa prerušilo. Zmeny zostali v editore; skús uloženie znovu.');
  }
  let payload;
  try {payload = await response.json();} catch {throw new Error('Server nevrátil očakávanú odpoveď. Skús uloženie znovu.');}
  if (!response.ok || !payload.ok) {
    const error = new Error(payload.error || 'Operácia sa nepodarila.'); error.status = response.status; throw error;
  }
  return payload.data;
}
function element(tag, className, text) {
  const node = document.createElement(tag); node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}
async function confirmAction(title, message, action) {
  const dialog = $('#confirm-dialog');
  $('#confirm-title').textContent = title; $('#confirm-text').textContent = message; $('#confirm-action').textContent = action;
  dialog.setAttribute('aria-labelledby', 'confirm-title'); dialog.setAttribute('aria-describedby', 'confirm-text');
  dialog.returnValue = 'cancel'; dialog.showModal();
  return new Promise(resolve => dialog.addEventListener('close', () => resolve(dialog.returnValue === 'confirm'), {once:true}));
}
$$('[data-close-dialog]').forEach(button => button.addEventListener('click', () => button.closest('dialog').close()));
function fillForm(form, values) {
  $$('[name]', form).forEach(input => { input.value = values[input.name] ?? ''; });
}
function formValues(form) {
  const data = Object.fromEntries(new FormData(form)); data.paymentDays = Number(data.paymentDays); return data;
}

let customers = boot.customers || [];
let customerBeingEdited = null;
let customerSaved;
function openCustomer(customer = null) {
  customerBeingEdited = customer;
  $('#customer-dialog-title').textContent = customer ? 'Upraviť zákazníka' : 'Nový zákazník';
  fillForm($('#customer-form'), customer || {country:'Slovensko', paymentDays:14, qrProfile:'sk'});
  errorAt('#customer-error', '');
  $('#customer-dialog').showModal(); $('#customer-name').focus();
}
$('#customer-form')?.addEventListener('submit', async event => {
  event.preventDefault(); const button = $('button[type="submit"]', event.target); button.disabled = true;
  try {
    const input = formValues(event.target);
    if (customerBeingEdited) { input.id = customerBeingEdited.id; input.version = customerBeingEdited.version; }
    const result = await api('customerSave', input);
    customers = [...customers.filter(item => item.id !== result.id), result];
    $('#customer-dialog').close(); notify('Zákazník je uložený.');
    if (customerSaved) customerSaved(result); else location.reload();
  } catch (error) {errorAt('#customer-error', error.message);} finally {button.disabled = false;}
});

if (boot.page === 'invoices') {
  let status = 'all';
  const filter = () => {
    const query = normalize($('#invoice-search').value); let shown = 0;
    $$('[data-invoice-row]').forEach(row => {
      const visible = (status === 'all' || row.dataset.status === status) && normalize(row.dataset.search).includes(query);
      row.hidden = !visible; if (visible) shown++;
    }); $('#list-empty').hidden = shown !== 0;
  };
  $('#invoice-search').addEventListener('input', filter);
  $$('[data-filter]').forEach(button => button.addEventListener('click', () => {
    status = button.dataset.filter;
    $$('[data-filter]').forEach(item => {const selected = item === button; item.setAttribute('aria-pressed', String(selected)); item.classList.toggle('btn-primary', selected); item.classList.toggle('btn-light', !selected);}); filter();
  })); filter();
}
if (boot.page === 'customers') {
  $('#add-customer').addEventListener('click', () => openCustomer());
  $$('[data-edit-customer]').forEach(button => button.addEventListener('click', () => openCustomer(customers.find(c => c.id === Number(button.dataset.editCustomer)))));
  $$('[data-archive-customer]').forEach(button => button.addEventListener('click', async () => {
    const customer = customers.find(c => c.id === Number(button.dataset.archiveCustomer));
    const archived = !customer.archived;
    if (!await confirmAction(archived ? 'Archivovať zákazníka?' : 'Obnoviť zákazníka?', `${customer.name}. ${archived ? 'Existujúce faktúry zostanú zachované.' : 'Firma bude opäť dostupná pri výbere odberateľa.'}`, archived ? 'Archivovať' : 'Obnoviť')) return;
    button.disabled = true;
    try {await api('customerArchive', {id:customer.id, version:customer.version, archived}); location.reload();} catch (error) {notify(error.message); button.disabled = false;}
  }));
  const filter = () => {
    const query = normalize($('#customer-search').value); let shown = 0;
    $$('[data-customer-row]').forEach(row => {const visible = normalize(row.dataset.search).includes(query); row.hidden = !visible; if (visible) shown++;});
    $('#customer-empty').hidden = shown !== 0;
  };
  $('#customer-search').addEventListener('input', filter); filter();
}
if (boot.page === 'settings') {
  let supplier = boot.supplier;
  fillForm($('#settings-form'), supplier);
  $('#settings-form').addEventListener('submit', async event => {
    event.preventDefault(); const button = $('button[type="submit"]', event.target); button.disabled = true;
    try {supplier = await api('settingsSave', {...formValues(event.target), version:supplier.version}); fillForm(event.target, supplier); errorAt('#settings-error', ''); notify('Údaje firmy sú uložené.');}
    catch (error) {errorAt('#settings-error', error.message);} finally {button.disabled = false;}
  });
}

if (boot.page === 'editor') {
  let invoice = structuredClone(boot.invoice);
  const issued = invoice.status === 'issued';
  const editing = boot.editing;
  const form = $('#invoice-form');
  $('#zoom-document')?.addEventListener('click', event => {const zoomed=$('.sheet-stage').classList.toggle('sheet-stage--zoomed');event.currentTarget.setAttribute('aria-pressed',String(zoomed));event.currentTarget.textContent=zoomed?'Zmenšiť':'Priblížiť';});
  let revision = 0, savedRevision = 0, timer, pendingSave = null, conflict = false, frozen = false;
  let refreshCustomer = false;

  function status(text) {$('#save-status').textContent = text;}
  function collect() {
    const data = {...invoice};
    for (const name of ['issueDate','deliveryDate','dueDate','orderNumber','note']) data[name] = $(`[name="${name}"]`, form).value;
    data.qrProfiles = $$('#qr-options input:checked').map(input => input.value);
    data.items = $$('.invoice-item').map(row => Object.fromEntries($$('[data-field]', row).map(input => [input.dataset.field, input.dataset.field === 'vatRate' ? Number(input.value) : input.value])));
    data.refreshCustomer = refreshCustomer; return data;
  }
  function renderTotals(totals) {
    $('#total-gross').textContent = money(totals.gross);
    $('#vat-summary').replaceChildren(...totals.groups.map(group => {
      const row = document.createElement('tr');
      [group.rate + ' %', money(group.net), money(group.vat), money(group.gross)].forEach(value => row.append(element('td','',value)));
      return row;
    }));
    $$('.invoice-item').forEach((row, i) => {$('[data-line-total]', row).textContent = money(totals.lines[i]?.net || 0);});
    renderMirrors();
  }
  function renderMirrors() {
    $$('.document-editable', form).forEach(wrapper => {
      const input = $('input, textarea, select', wrapper);
      let value = input.value;
      if (input.type === 'date' && value) value = value.split('-').reverse().join('.');
      if (input.dataset.field === 'unitPrice') {
        try {value = money(Number(decimalUnits(value, 2)));} catch {}
      }
      if (input.dataset.field === 'quantity' && value) {
        const number = Number(value.replace(',', '.').replace(/ /g,''));
        if(Number.isFinite(number)) value = new Intl.NumberFormat('sk-SK',{maximumFractionDigits:3}).format(number);
      }
      if (input.dataset.field === 'vatRate') value += ' %';
      $('[data-display]',wrapper).textContent = value || input.placeholder || '—';
      wrapper.classList.toggle('document-editable--empty',!input.value);
    });
  }
  function renderQrPlaceholders() {
    const row = $('.document-payment tr');
    $$('.document-qr',row).forEach(cell=>cell.remove());
    const labels={sk:'PAY by square · Slovensko',cz:'QR Platba · Česko',epc:'SEPA QR · Európa'};
    $$('#qr-options input:checked').forEach(input=>{
      const cell=element('td','document-qr'); cell.dataset.qrPlaceholder='';
      const image=document.createElement('img'); image.className='document-qr__image';image.width=151;image.height=151;image.src=boot.qrPlaceholder;image.alt=issued?'QR po uložení':'QR po vystavení';
      cell.append(image,document.createElement('br'),element('b','',labels[input.value])); row.append(cell);
    });
    const hint=$('#qr-instruction'); if(hint) hint.hidden=$$('#qr-options input:checked').length===0;
  }
  function recalculate() {
    try {renderTotals(calculate(collect().items));} catch {$('#total-gross').textContent = '—';}
  }
  function setFrozen(value) {
    if (value) Dropdown.getInstance($('#invoice-more'))?.hide();
    frozen = value;
    $$('input, textarea, select, button', form).forEach(input => {input.disabled = value || !editing;});
    $$('#qr-options input').forEach(input => {input.disabled = value || !editing;});
    $$('#editor-actions button, #preview-pdf, #delete-invoice').forEach(button => {button.disabled = value;});
    $$('.invoice-item__remove').forEach(button => {button.disabled = value || !editing || $$('.invoice-item').length === 1;});
  }
  function changed() {
    if (!editing || frozen) return;
    revision++; status('Neuložené zmeny…'); recalculate(); clearTimeout(timer);
    if (!issued) timer = setTimeout(() => {save().catch(() => {});}, 700);
  }
  async function save() {
    clearTimeout(timer);
    if (conflict) throw new Error('Faktúra sa zmenila v inom okne. Najprv obnov stránku.');
    if (pendingSave) return pendingSave;
    if (invoice.id && savedRevision === revision) return invoice;
    pendingSave = (async () => {
      try {
        while (savedRevision < revision || !invoice.id) {
          const capturedRevision = revision; const payload = collect();
          status('Ukladám…');
          const result = await api('invoiceSave', payload);
          invoice = {...invoice, id:result.id, version:result.version, supplier:result.supplier};
          if (invoice.customerId === result.customerId) invoice.customer = result.customer;
          if (revision === capturedRevision) {refreshCustomer = false; renderTotals(result.totals);}
          savedRevision = capturedRevision;
          if (!issued) history.replaceState(null, '', `/faktury/${result.id}`);
        }
        status('Všetky zmeny sú uložené'); errorAt('#editor-error', '');
        return invoice;
      } catch (error) {
        if (error.status === 409) conflict = true;
        status('Zmeny sa nepodarilo uložiť'); errorAt('#editor-error', error.message); throw error;
      } finally {pendingSave = null;}
    })();
    return pendingSave;
  }
  function renderCustomer() {
    if(!editing) return;
    const target = $('#customer-details'); target.replaceChildren();
    const customer = invoice.customer;
    if (!customer) {target.append(element('p','document-empty','Odberateľ nie je vybraný'));return;}
    target.append(element('h3','document-company',customer.name));
    const address = element('p','');
    [customer.street, `${customer.zip} ${customer.city}`, customer.country].forEach((line, i) => {if(i) address.append(document.createElement('br')); address.append(document.createTextNode(line));}); target.append(address);
    [['IČO',customer.ico],['DIČ',customer.dic],['IČ DPH',customer.vatId]].filter(([,value])=>value).forEach(([label,value]) => target.append(element('p','',`${label}: ${value}`)));
  }
  function selectCustomer(customer) {
    invoice.customerId = customer.id; invoice.customer = customer; refreshCustomer = true;
    $$('#qr-options input').forEach(input => {input.checked = input.value === customer.qrProfile;});
    const issueDate = $('#issueDate').value;
    if (issueDate) {const date = new Date(`${issueDate}T12:00:00Z`); date.setUTCDate(date.getUTCDate() + customer.paymentDays); $('#dueDate').value = date.toISOString().slice(0,10);}
    $('#customer-picker').close(); renderCustomer(); renderQrPlaceholders(); changed();
  }
  function renderPicker() {
    const search = normalize($('#picker-search').value);
    const matches = customers.filter(c => !c.archived && normalize(`${c.name} ${c.ico}`).includes(search));
    $('#picker-results').replaceChildren(...matches.map(customer => {
      const button = element('button','customer-choice btn text-start w-100 border-bottom py-3'); button.type='button';
      button.append(element('span','d-block fw-semibold',customer.name), element('span','d-block small text-secondary',`${customer.city} · IČO ${customer.ico || '—'}`));
      button.addEventListener('click', () => selectCustomer(customer)); return button;
    }));
    if (!matches.length) $('#picker-results').append(element('p','text-secondary py-3 mb-0','Žiadny zákazník nezodpovedá vyhľadávaniu.'));
  }
  function openPicker() {$('#picker-search').value=''; renderPicker(); $('#customer-picker').showModal(); $('#picker-search').focus();}
  $('#choose-customer')?.addEventListener('click', openPicker);
  $('#picker-search').addEventListener('input',renderPicker);
  $('#picker-add').addEventListener('click', () => {$('#customer-picker').close(); openCustomer();});
  customerSaved = selectCustomer;

  function renderItems(items, focusIndex = null) {
    const template=$('#item-template');
    $('#invoice-items').replaceChildren(...items.map((item,index)=>{
      const row=template.content.querySelector('tr').cloneNode(true);
      $$('[data-field]',row).forEach(input=>{
        const field=input.dataset.field;
        input.value=item[field];input.id=`item-${index}-${field}`;
        input.setAttribute('aria-label',input.getAttribute('aria-label').replace(/položka \d+$/,`položka ${index+1}`));
        input.disabled=frozen;
      });
      const remove=$('.document-remove',row); remove.setAttribute('aria-label',`Odstrániť položku ${index+1}`);remove.disabled=items.length===1;
      remove.addEventListener('click',()=>{const updated=collect().items;updated.splice(index,1);renderItems(updated);changed();$('#add-item').focus();});
      return row;
    }));
    recalculate();
    if(focusIndex!==null) $(`#item-${focusIndex}-description`).focus();
  }
  $('#add-item')?.addEventListener('click', () => {const items=collect().items; if(items.length>=100) {notify('Faktúra môže mať najviac 100 položiek.');return;} items.push({description:'',quantity:'1',unit:'ks',unitPrice:'0.00',vatRate:23}); renderItems(items,items.length-1); changed();});
  form.addEventListener('submit',event => event.preventDefault());
  form.addEventListener('input',changed);
  $$('#qr-options input').forEach(input => {
    input.checked=invoice.qrProfiles.includes(input.value); input.disabled=!editing;
    input.addEventListener('change', () => {if($$('#qr-options input:checked').length>2) {input.checked=false; notify('Do PDF môžeš pridať najviac dva QR kódy.');return;} renderQrPlaceholders(); changed();});
  });
  if(editing) {renderItems(invoice.items); renderCustomer(); renderQrPlaceholders();}
  if(issued && !editing) {
    $$('input,textarea,select',form).forEach(input => {input.disabled=true;});
    $('#copy-invoice').addEventListener('click', async event => {event.currentTarget.disabled=true; try {const copy=await api('invoiceCopy',{id:invoice.id}); location.href=`/faktury/${copy.id}`;} catch(error) {notify(error.message); $('#copy-invoice').disabled=false;}});
    if(new URLSearchParams(location.search).get('download')==='1') {const anchor=document.createElement('a'); anchor.href=`/faktury/${invoice.id}/pdf?download=1`; anchor.download=''; document.body.append(anchor); anchor.click(); anchor.remove(); history.replaceState(null,'',location.pathname);}
  }
  $('#save-invoice')?.addEventListener('click',async () => {
    if(issued && !form.reportValidity()) return;
    setFrozen(true);
    try {
      await save();
      if(issued) location.href=`/faktury/${invoice.id}`;
      else notify('Koncept je uložený.');
    } catch {} finally {setFrozen(false);}
  });
  if(!issued) {
    $('#issue-invoice').addEventListener('click', async () => {
      if(!form.reportValidity()) return;
      if(!invoice.customerId) {errorAt('#editor-error','Vyber odberateľa zo zoznamu zákazníkov.'); $('#choose-customer').focus(); return;}
      setFrozen(true);
      try {
        await save(); const data=collect();
        if(!await confirmAction('Vystaviť faktúru?', `${invoice.customer?.name || 'Bez odberateľa'} · ${money(calculate(data.items).gross)}. Faktúra dostane číslo a uloží sa konečné PDF.`, 'Vystaviť a stiahnuť')) return;
        const result=await api('invoiceIssue',{id:invoice.id,version:invoice.version});
        savedRevision=revision; location.href=`/faktury/${result.id}?download=1`;
      } catch(error) {errorAt('#editor-error',error.message);} finally {setFrozen(false);}
    });
    $('#preview-pdf').addEventListener('click', async () => {
      const preview=window.open('about:blank','_blank'); setFrozen(true);
      try {await save(); if(preview) preview.location.href=`/faktury/${invoice.id}/pdf`; else location.href=`/faktury/${invoice.id}/pdf`;}
      catch {preview?.close();} finally {setFrozen(false);}
    });
  }
  $('#delete-invoice').addEventListener('click',async () => {
    setFrozen(true);
    try {
      if(pendingSave) await pendingSave;
      clearTimeout(timer);
      const description=invoice.number ? `Faktúra ${invoice.number} aj jej uložené PDF sa vymažú. Jej číslo sa znova nepoužije.` : 'Rozpracovaná faktúra sa vymaže. Číselný rad zostane nezmenený.';
      if(!await confirmAction('Vymazať faktúru?', description, 'Vymazať')) return;
      if(invoice.id) await api('invoiceDelete',{id:invoice.id,version:invoice.version});
      savedRevision=revision; location.href='/';
    } catch(error) {errorAt('#editor-error',error.message);}
    finally {
      setFrozen(false);
      if(!issued && revision!==savedRevision && !conflict) timer=setTimeout(()=>{save().catch(()=>{});},700);
    }
  });
  if(editing) window.addEventListener('beforeunload',event => {if(revision!==savedRevision || pendingSave) {event.preventDefault();event.returnValue='';}});
}

initEmail({boot,api,notify,errorAt,fillForm});
