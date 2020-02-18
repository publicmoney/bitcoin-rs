<script>
    import {request} from '../jsonrpc'
    export default {
        name: 'mempool',
        data: function() {
            return {
                info: 'loading'
            };
        },
        methods: {
            loadInfo: async function() {
                this.info = await request('getmempoolinfo', []);
            }
        },
        mounted: function () {
            this.loadInfo();
        }
    }
</script>

<template>
    <div id="main">
        <h2>Memory Pool Information</h2>
        <ul id="info">
            <li v-for="(value, name) in info" v-bind:key="name">
                {{ name }}: {{ value }}
            </li>
        </ul>
    </div>
</template>

<style>
    #main {
        text-align: left;
    }
</style>