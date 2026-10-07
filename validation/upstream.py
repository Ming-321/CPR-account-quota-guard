"""仅供隔离宿主验证的合成上游，不使用真实账号"""
import json, time, threading, base64, hashlib, struct
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse

lock = threading.Lock()
state = {"A": {"used": 80, "reset": int(time.time()) + 300000}, "B": {"used": 20, "reset": int(time.time()) + 300000}}
events = []
class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    def log_message(self, *args): pass
    def send(self, status, value, content_type="application/json", headers=None):
        body = value.encode() if isinstance(value, str) else json.dumps(value).encode()
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        for k,v in (headers or {}).items(): self.send_header(k,str(v))
        self.end_headers(); self.wfile.write(body)
    def do_GET(self):
        path = urlparse(self.path).path
        if self.headers.get("Upgrade", "").lower() == "websocket":
            return self.websocket(path)
        if path == "/control": return self.send(200, {"state": state, "events": events})
        account = self.headers.get("chatgpt-account-id", "A")
        with lock: events.append({"path":path,"account":account,"at":time.time()})
        item = state.get(account, state["A"])
        if path.endswith("/usage"):
            return self.send(200, {"plan_type":"pro","rate_limit":{"allowed":True,"limit_reached":False,"primary_window":{"used_percent":item["used"],"limit_window_seconds":604800,"reset_at":item["reset"]}}})
        if "models" in path:
            return self.send(200, {"models":[{"slug":"gpt-5.4","display_name":"GPT-5.4","supported_in_api":True,"visibility":"list","priority":0,"input_modalities":["text"],"supported_reasoning_levels":[{"effort":"medium","description":"medium"}],"default_reasoning_level":"medium","context_window":272000}]})
        return self.send(404, {"error":"unknown synthetic endpoint"})
    def websocket(self, path):
        key=self.headers["Sec-WebSocket-Key"]
        accept=base64.b64encode(hashlib.sha1((key+"258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()).decode()
        self.send_response(101);self.send_header("Upgrade","websocket");self.send_header("Connection","Upgrade");self.send_header("Sec-WebSocket-Accept",accept);self.end_headers()
        account=self.headers.get("chatgpt-account-id","A")
        def emit(value, opcode=1):
            data=json.dumps(value).encode() if opcode==1 else value
            n=len(data);head=bytes([0x80|opcode,n]) if n<126 else bytes([0x80|opcode,126])+struct.pack("!H",n)
            self.wfile.write(head+data);self.wfile.flush()
        try:
            while True:
                first,second=self.rfile.read(2);n=second&127
                if n==126:n=struct.unpack("!H",self.rfile.read(2))[0]
                elif n==127:n=struct.unpack("!Q",self.rfile.read(8))[0]
                mask=self.rfile.read(4) if second&128 else None
                data=self.rfile.read(n)
                if mask:data=bytes(v^mask[i%4] for i,v in enumerate(data))
                if first&15==8:emit(data,8);return
                if first&15==9:emit(data,10);continue
                if first&15!=1:continue
                command=json.loads(data)
                if command.get("type")!="response.create":continue
                with lock:events.append({"path":path,"account":account,"at":time.time(),"transport":"websocket"})
                response={"id":"resp_"+str(time.time_ns()),"object":"response","status":"completed","model":"gpt-5.4","output":[],"usage":{"input_tokens":1,"output_tokens":1,"total_tokens":2}}
                emit({"type":"response.created","response":{**response,"status":"in_progress"}})
                emit({"type":"response.completed","response":response})
        except (ValueError, EOFError, ConnectionError, OSError):
            return
    def do_POST(self):
        body = self.rfile.read(int(self.headers.get("Content-Length",0)))
        path = urlparse(self.path).path
        if path == "/control":
            command = json.loads(body)
            with lock:
                for k,v in command.get("accounts",{}).items(): state.setdefault(k,{}).update(v)
                if command.get("clear"): events.clear()
            return self.send(200, {"ok":True})
        account = self.headers.get("chatgpt-account-id","A")
        with lock: events.append({"path":path,"account":account,"at":time.time()})
        item = state.get(account,state["A"])
        if path.endswith("/responses"):
            if item.get("fail"):
                item["fail"] -= 1
                return self.send(503, {"error":{"message":"synthetic capacity rejection","type":"server_is_overloaded","code":"server_is_overloaded"}})
            response = {"id":"resp_"+str(time.time_ns()),"object":"response","created_at":int(time.time()),"status":"completed","model":"gpt-5.4","output":[{"type":"message","id":"msg_mock","role":"assistant","status":"completed","content":[{"type":"output_text","text":"quota guard validation","annotations":[]}]}],"usage":{"input_tokens":1,"output_tokens":1,"total_tokens":2}}
            frames = [{"type":"response.created","response":{**response,"status":"in_progress","output":[]}},{"type":"response.output_text.delta","item_id":"msg_mock","output_index":0,"content_index":0,"delta":"quota guard validation"},{"type":"response.completed","response":response}]
            headers = {"x-codex-primary-used-percent":item["used"],"x-codex-primary-window-minutes":10080,"x-codex-primary-reset-at":item["reset"],"x-codex-plan-type":"pro"}
            return self.send(200, "".join("event: "+f["type"]+"\ndata: "+json.dumps(f)+"\n\n" for f in frames), "text/event-stream", headers)
        return self.send(404, {"error":"unknown synthetic endpoint"})
ThreadingHTTPServer(("0.0.0.0",9081),Handler).serve_forever()
