import logging
from django.conf import settings
from django.http import JsonResponse, HttpResponseBadRequest
from django.shortcuts import render
from .ssh_handler import get_server_usage

logger = logging.getLogger(__name__)

def monitor(request):
    return render(request, "monitor.html", {"servers": settings.SERVERS})

import logging
from django.conf import settings
from django.http import JsonResponse, HttpResponseBadRequest
from django.shortcuts import render
from .ssh_handler import get_server_usage

logger = logging.getLogger(__name__)

def monitor(request):
    return render(request, "monitor.html", {"servers": settings.SERVERS})

def get_usage(request):
    try:
        server_index = request.GET.get("server_index")
        if server_index is None:
            return HttpResponseBadRequest("Missing server_index parameter")

        try:
            index = int(server_index)
            if index < 0 or index >= len(settings.SERVERS):
                return HttpResponseBadRequest("Invalid server_index")
        except ValueError:
            return HttpResponseBadRequest("Invalid server_index parameter")

        server_config = settings.SERVERS[index]
        logger.debug(f"Server Config for index {index}: {server_config}")
        usage = get_server_usage(server_config)
        logger.debug(f"Usage data: {usage}")

        if "error" in usage:
            logger.error(f"Error retrieving usage for {server_config.get('hostname')}: {usage['error']}")

        display_hostname = server_config.get("then", {}).get("hostname") if "then" in server_config else server_config.get("hostname")
        logger.debug(f"Display Hostname: {display_hostname}")

        return JsonResponse({
            **usage,
            "server": display_hostname
        })

    except Exception as e:
        logger.exception("Exception in get_usage view")
        return JsonResponse({"error": "Internal server error"}, status=500)