"""使用真实 CPR WebSocket 入口验证同一连接的后续轮次"""
import base64, hashlib, json, os, socket, struct, time
from pathlib import Path
from urllib.parse import urlparse
from client import *

def exact(sock,n):
    data=b""
    while len(data)<n:
        part=sock.recv(n-len(data))
        if not part:raise EOFError("socket closed")
        data+=part
    return data
def send(sock,value,opcode=1):
    data=json.dumps(value).encode() if opcode==1 else value
    mask=os.urandom(4);n=len(data)
    head=bytes([0x80|opcode,0x80|n]) if n<126 else bytes([0x80|opcode,0x80|126])+struct.pack("!H",n)
    sock.sendall(head+mask+bytes(v^mask[i%4] for i,v in enumerate(data)))
def receive(sock):
    while True:
        first,second=exact(sock,2);n=second&127
        if n==126:n=struct.unpack("!H",exact(sock,2))[0]
        elif n==127:n=struct.unpack("!Q",exact(sock,8))[0]
        mask=exact(sock,4) if second&128 else None
        data=exact(sock,n)
        if mask:data=bytes(v^mask[i%4] for i,v in enumerate(data))
        if first&15==9:send(sock,data,10);continue
        if first&15==8:raise EOFError("websocket closed")
        return json.loads(data)
def turn(sock,previous=None):
    value={"type":"response.create","model":"gpt-5.4","input":"test"}
    if previous:value["previous_response_id"]=previous
    send(sock,value)
    for _ in range(20):
        frame=receive(sock)
        if frame.get("type") in ("error","response.failed","response.completed"):return frame
    raise AssertionError("no terminal event")

if __name__=="__main__":
    login()
    ids=json.loads(Path(os.environ["CPR_TEST_IDS"]).read_text())
    account=ids["A"]["account"]
    current=api("GET",extension()+"state")["guards"].get(account,{}).get("guard",{})
    reset=(current.get("cycle") or {}).get("reset")
    control({"accounts":{"A":{"used":80,**({"reset":reset//1000} if reset else {})}}})
    api("POST","/api/admin/accounts/quota/refresh",{"accountId":account})
    s=api("GET",extension()+"state")
    api("POST",extension()+"settings",{"version":s["version"],"account_id":account,"enabled":True,"rule":{"mode":"fixed","threshold":15}})
    host=urlparse(BASE)
    key=base64.b64encode(os.urandom(16)).decode()
    with socket.create_connection((host.hostname,host.port),timeout=10) as sock:
        sock.sendall(("GET /v1/responses HTTP/1.1\r\nHost: "+host.netloc+"\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: "+key+"\r\nAuthorization: Bearer "+ids["A"]["key"]+"\r\n\r\n").encode())
        head=b""
        while not head.endswith(b"\r\n\r\n"):head+=exact(sock,1)
        assert b"101" in head,head
        first=turn(sock)
        assert first["type"]=="response.completed",first
        response_id=first["response"]["id"]
        time.sleep(.2)
        control({"accounts":{"A":{"used":90}}})
        api("POST","/api/admin/accounts/quota/refresh",{"accountId":account})
        before=len([e for e in control()["events"] if e["path"].endswith("/responses")])
        second=turn(sock,response_id)
        assert second["type"]=="error",second
        assert second["status"]==403 and second["error"]["code"]=="policy_denied",second
        assert len([e for e in control()["events"] if e["path"].endswith("/responses")])==before
        control({"accounts":{"A":{"used":80}}})
        api("POST","/api/admin/accounts/quota/refresh",{"accountId":account})
        time.sleep(.3)
        third=turn(sock,response_id)
        assert third["type"]=="response.completed",third
    print("PASS 同一 WebSocket 首轮成功、续接被保护拦截、额度恢复后继续成功")
