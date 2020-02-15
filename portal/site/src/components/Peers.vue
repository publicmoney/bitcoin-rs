<script>
    import {request} from '../jsonrpc'
    export default {
        name: 'peers',
        data: function() {
            return {
                peers: 'loading',
                numInboundPeers: 0,
                numOutboundPeers: 0
            };
        },
        methods: {
            loadInfo: async function() {
                this.peers = await request('getpeerinfo', []);
                this.numInboundPeers = this.peers.filter(p => p.inbound === true).length;
                this.numOutboundPeers = this.peers.filter(p => p.inbound === false).length;
            }
        },
        mounted: function () {
            this.loadInfo();
        }
    }
</script>

<template>
    <div id="main">
        <h2>Peer Connections</h2>
        <h3>Inbound: {{numInboundPeers}}</h3>
        <h3>Outbound: {{numOutboundPeers}}</h3>
        <ul id="peers">
            <div id="peer" v-for="(v, n) in peers" v-bind:key="n">
                <li v-for="(value, name) in v" v-bind:key="name.id">
                    {{ name }}: {{ value }}
                </li>
            </div>
        </ul>
    </div>
</template>

<style>
    #main {
        text-align: left;
    }
    #peer {
        margin-bottom: 30px;
    }
</style>