"""Independent Poppler/zbar checks; run in Docker's verify target on synthetic artifacts."""
from pathlib import Path
from email import policy
from email.parser import BytesParser
import base64
import binascii
import json
import lzma
import re
import subprocess
import sys
import xml.etree.ElementTree as ET

folder=Path(sys.argv[1] if len(sys.argv)>1 else '/checks')

def decode(path):
    result=subprocess.run(['zbarimg','--quiet','--xml',str(path)],capture_output=True,check=True,timeout=30)
    root=ET.fromstring(result.stdout)
    payloads=[]
    for symbol in root.findall('.//{*}symbol'):
        data=symbol.find('{*}data')
        payloads.append(base64.b64decode(data.text).decode() if data.get('format')=='base64' else data.text)
    return payloads

def pay_fields(payload):
    data=base64.b32hexdecode(payload+'='*((-len(payload))%8))
    assert data[:2]==b'\0\0'
    expected=int.from_bytes(data[2:4],'little')
    checked=lzma.decompress(data[4:],format=lzma.FORMAT_RAW,filters=[{'id':lzma.FILTER_LZMA1,'dict_size':131072,'lc':3,'lp':0,'pb':2}])
    assert len(checked)==expected
    assert int.from_bytes(checked[:4],'little')==binascii.crc32(checked[4:])
    fields=checked[4:].decode().split('\t')
    assert len(fields)==19 and fields[5]==''
    return fields

payloads=json.loads((folder/'qr-payloads.json').read_text())
for profile,payload in payloads.items():
    assert decode(folder/f'qr-{profile}.png')==[payload],profile
    if profile=='sk':pay_fields(payload)
    print('PASS independently decoded QR image',profile)

pdf=folder/'invoice-sk-epc.pdf'
subprocess.run(['pdftoppm','-png','-r','160','-singlefile',str(pdf),str(folder/'printed-invoice')],check=True,timeout=30)
decoded=decode(folder/'printed-invoice.png')
assert sorted(decoded)==sorted([payloads['sk'],payloads['epc']])
print('PASS SK/EPC QR codes decoded from rendered PDF page')

long=folder/'invoice-multipage.pdf'
info=subprocess.check_output(['pdfinfo',str(long)],text=True)
pages=int(re.search(r'^Pages:\s+(\d+)',info,re.M)[1])
assert pages>1 and 'Test dodávateľ s. r. o.' in info
text=subprocess.check_output(['pdftotext','-layout',str(long),'-'],text=True)
for i in range(1,61):assert f'Položka {i:02}' in text,i
assert 'Koniec poznámky.' in text
for i in range(1,pages+1):assert re.search(rf'\b{i}\s*/\s*{pages}\b',text),i
assert 'file://' not in text and 'about:blank' not in text
for page in [1,pages]:
    subprocess.run(['pdftoppm','-png','-scale-to','1200','-f',str(page),'-l',str(page),'-singlefile',str(long),str(folder/f'long-page-{page}')],check=True,timeout=30)
print(f'PASS all 60 rows, multiline note, metadata and page counters across {pages} PDF pages')

mail_path=folder/'email-captured.eml'
if mail_path.exists():
    expected=json.loads((folder/'email-expectations.json').read_text())['preview']
    message=BytesParser(policy=policy.default).parsebytes(mail_path.read_bytes())
    pdfs=[p for p in message.walk() if p.get_content_type()=='application/pdf']
    assert len(pdfs)==1 and pdfs[0].get_filename()==expected['attachment']
    assert pdfs[0].get_payload(decode=True)==(folder/'email-attached-original.pdf').read_bytes()
    html=message.get_body(preferencelist=('html',)).get_content()
    images=[p for p in message.walk() if p.get_content_type()=='image/png']
    assert len(images)==2
    amount=expected['amount'].replace(' ','').replace('\u00a0','').replace('€','').strip().replace(',','.')
    for i,part in enumerate(images):
        cid=part['Content-ID'].strip('<>');assert 'cid:'+cid in html
        image=folder/f'email-inline-{i}.png';image.write_bytes(part.get_payload(decode=True))
        payload=decode(image)[0]
        if payload.startswith('BCD\n'):
            fields=payload.split('\n');assert fields[7]=='EUR'+amount and fields[-1]=='Faktura '+expected['number']
        else:
            fields=pay_fields(payload);assert fields[3]==amount and fields[6]==expected['number']
            assert fields[12]==expected['iban'].replace(' ','')
    print('PASS MIME attachment byte equality, inline Content-ID images and independently decoded payment data')
