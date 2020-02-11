<script>
    import {request} from '../jsonrpc'
    export default {
        name: 'info',
        data: function() {
            return {
                blockchainInfo: 'loading'
            };
        },
        methods: {
            loadInfo: async function() {
                this.blockchainInfo = await request('getblockchaininfo', []);
            }
        },
        mounted: function () {
            this.loadInfo();
        }
    }
</script>

<template>
    <div id="info">
        <h2>Blockchain Info</h2>
        <ul id="bc">
            <li v-for="(value, name) in blockchainInfo" v-bind:key="name">
                {{ name }}: {{ value }}
            </li>
        </ul>
    </div>
</template>

<style>
    #info {
        text-align: left;
    }
</style>