export async function request(method, params) {
    let response = await fetch('http://localhost:8332',{
        method: 'POST',
        mode: 'cors',
        url: "http://localhost:8332",
        json: false,
        headers: {
            'Content-Type': 'application/json'
        },
        destination: '',
        body: JSON.stringify({
            id: 'portal',
            jsonrpc: '2.0',
            method: method,
            params: params,
        })
    });

    return (await response.json()).result;
}