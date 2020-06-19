<template>
    <div id="main">
        <h2>Network Info</h2>
		<ul id="network">
			<li v-for="(value, name) in network" v-bind:key="name">
				{{ name }}: {{ value }}
			</li>
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

<script>
    import {request} from '../jsonrpc'
    export default {
        name: 'network',
        data: function() {
            return {
                network: 'loading',
            };
        },
        methods: {
            loadInfo: async function() {
                this.network = await request('getnetworkinfo', []);
            }
        },
        mounted: function () {
            this.loadInfo();
        }
    }
</script>

